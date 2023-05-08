// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

use std::cell::RefCell;
use std::pin::Pin;
use std::rc::{Rc, Weak};

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::{euclid, rendering_metrics_collector::RenderingMetricsCollector};
use i_slint_core::lengths::{
    LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PhysicalPx, ScaleFactor,
};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::Renderer;
use i_slint_core::window::{WindowAdapter, WindowInner};
use i_slint_core::Brush;

type PhysicalLength = euclid::Length<f32, PhysicalPx>;
type PhysicalRect = euclid::Rect<f32, PhysicalPx>;
type PhysicalSize = euclid::Size2D<f32, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<f32, PhysicalPx>;

use self::itemrenderer::CanvasRc;

mod fonts;
mod images;
mod itemrenderer;

/// Use the FemtoVG renderer when implementing a custom Slint platform where you deliver events to
/// Slint and want the scene to be rendered using OpenGL and the FemtoVG renderer.
pub struct FemtoVGRenderer {
    window_adapter_weak: Weak<dyn WindowAdapter>,
    canvas: CanvasRc,
    graphics_cache: itemrenderer::ItemGraphicsCache,
    texture_cache: RefCell<images::TextureCache>,
    rendering_metrics_collector: RefCell<Option<Rc<RenderingMetricsCollector>>>,
}

impl FemtoVGRenderer {
    /// Creates a new renderer is associated with the provided window adapter.
    /// This assumes that the correct OpenGL context is current.
    /// The provided `get_proc_address` argument needs to provide the OpenGL ES API,
    /// typically this is forwarded to for example eglGetProcAddress.
    /// For WASM builds, the provided canvas needs to be inserted in the DOM.
    pub fn new(
        window_adapter_weak: &Weak<dyn WindowAdapter>,
        #[cfg(not(target_arch = "wasm32"))] get_proc_address: impl FnMut(
            &std::ffi::CStr,
        )
            -> *const std::ffi::c_void,
        #[cfg(target_arch = "wasm32")] canvas: &web_sys::HtmlCanvasElement,
    ) -> Result<Self, PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        let gl_renderer =
            unsafe { femtovg::renderer::OpenGl::new_from_function_cstr(get_proc_address).unwrap() };

        #[cfg(target_arch = "wasm32")]
        let gl_renderer = match femtovg::renderer::OpenGl::new_from_html_canvas(canvas) {
            Ok(gl_renderer) => gl_renderer,
            Err(_) => {
                use wasm_bindgen::JsCast;

                // I don't believe that there's a way of disabling the 2D canvas.
                let context_2d = canvas
                    .get_context("2d")
                    .unwrap()
                    .unwrap()
                    .dyn_into::<web_sys::CanvasRenderingContext2d>()
                    .unwrap();
                context_2d.set_font("20px serif");
                // We don't know if we're rendering on dark or white background, so choose a "color" in the middle for the text.
                context_2d.set_fill_style(&wasm_bindgen::JsValue::from_str("red"));
                context_2d
                    .fill_text("Slint requires WebGL to be enabled in your browser", 0., 30.)
                    .unwrap();
                panic!("Cannot proceed without WebGL - aborting")
            }
        };

        let femtovg_canvas = femtovg::Canvas::new_with_text_context(
            gl_renderer,
            self::fonts::FONT_CACHE.with(|cache| cache.borrow().text_context.clone()),
        )
        .unwrap();
        let canvas = Rc::new(RefCell::new(femtovg_canvas));

        Ok(Self {
            window_adapter_weak: window_adapter_weak.clone(),
            canvas,
            graphics_cache: Default::default(),
            texture_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
        })
    }

    /// Notifiers the renderer that the underlying window is becoming visible.
    pub fn show(&self) {
        *self.rendering_metrics_collector.borrow_mut() = RenderingMetricsCollector::new(
            self.window_adapter_weak.clone(),
            &format!("FemtoVG renderer"),
        );
    }

    /// Notifiers the renderer that the underlying window will be hidden soon.
    pub fn hide(&self) {
        self.rendering_metrics_collector.borrow_mut().take();
    }

    /// Render the scene using OpenGL. This function assumes that the context is current.
    pub fn render(
        &self,
        size: PhysicalWindowSize,
        mut before_rendering_callback: Option<impl FnOnce() -> Result<(), PlatformError>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let width = size.width;
        let height = size.height;

        let window_adapter = self.window_adapter_weak.upgrade().unwrap();
        let window = WindowInner::from_pub(window_adapter.window());

        window.draw_contents(|components| -> Result<(), PlatformError> {
            let window_background_brush = window.window_item().map(|w| w.as_pin_ref().background());

            {
                let mut femtovg_canvas = self.canvas.borrow_mut();
                // We pass 1.0 as dpi / device pixel ratio as femtovg only uses this factor to scale
                // text metrics. Since we do the entire translation from logical pixels to physical
                // pixels on our end, we don't need femtovg to scale a second time.
                femtovg_canvas.set_size(width, height, 1.0);

                // Clear with window background if it is a solid color otherwise it will drawn as gradient
                if let Some(Brush::SolidColor(clear_color)) = window_background_brush {
                    femtovg_canvas.clear_rect(
                        0,
                        0,
                        width,
                        height,
                        self::itemrenderer::to_femtovg_color(&clear_color),
                    );
                }
            }

            if let Some(callback) = before_rendering_callback.take() {
                let mut femtovg_canvas = self.canvas.borrow_mut();
                // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
                // the back buffer, in order to allow the callback to provide its own rendering of the background.
                // femtovg's clear_rect() will merely schedule a clear call, so flush right away to make it immediate.

                femtovg_canvas.flush();

                femtovg_canvas.set_size(width, height, 1.0);
                drop(femtovg_canvas);

                callback()?;
            }

            let window_adapter = self.window_adapter_weak.upgrade().unwrap();

            let mut item_renderer = self::itemrenderer::GLItemRenderer::new(
                &self.canvas,
                &self.graphics_cache,
                &self.texture_cache,
                window_adapter.window(),
                width,
                height,
            );

            // Draws the window background as gradient
            match window_background_brush {
                Some(Brush::SolidColor(..)) | None => {}
                Some(brush @ _) => {
                    item_renderer.draw_rect(
                        i_slint_core::lengths::logical_size_from_api(
                            size.to_logical(window.scale_factor()),
                        ),
                        brush,
                    );
                }
            }

            for (component, origin) in components {
                i_slint_core::item_rendering::render_component_items(
                    component,
                    &mut item_renderer,
                    *origin,
                );
            }

            if let Some(collector) = &self.rendering_metrics_collector.borrow().as_ref() {
                collector.measure_frame_rendered(&mut item_renderer);
            }

            self.canvas.borrow_mut().flush();

            // Delete any images and layer images (and their FBOs) before making the context not current anymore, to
            // avoid GPU memory leaks.
            self.texture_cache.borrow_mut().drain();
            drop(item_renderer);
            Ok(())
        })
    }
}

impl Renderer for FemtoVGRenderer {
    fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<LogicalLength>,
        scale_factor: ScaleFactor,
    ) -> LogicalSize {
        crate::fonts::text_size(&font_request, scale_factor, text, max_width)
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        pos: LogicalPoint,
    ) -> usize {
        let window_adapter = match self.window_adapter_weak.upgrade() {
            Some(window) => window,
            None => return 0,
        };

        let window = WindowInner::from_pub(window_adapter.window());

        let scale_factor = ScaleFactor::new(window.scale_factor());
        let pos = pos * scale_factor;
        let text = text_input.text();

        let mut result = text.len();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width.get() <= 0. || height.get() <= 0. || pos.y < 0. {
            return 0;
        }

        let font = crate::fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input.font_request(&window_adapter),
                scale_factor,
                &text_input.text(),
            )
        });

        let visual_representation = text_input.visual_representation(None);

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        let text_context =
            crate::fonts::FONT_CACHE.with(|cache| cache.borrow().text_context.clone());
        let font_height = text_context.measure_font(&paint).unwrap().height();
        crate::fonts::layout_text_lines(
            &visual_representation.text,
            &font,
            PhysicalSize::from_lengths(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::Clip,
            text_input.single_line(),
            &paint,
            |line_text, line_pos, start, metrics| {
                if (line_pos.y..(line_pos.y + font_height)).contains(&pos.y) {
                    let mut current_x = 0.;
                    for glyph in &metrics.glyphs {
                        if line_pos.x + current_x + glyph.advance_x / 2. >= pos.x {
                            result = start + glyph.byte_index;
                            return;
                        }
                        current_x += glyph.advance_x;
                    }
                    result = start + line_text.trim_end().len();
                }
            },
        );

        visual_representation.map_byte_offset_from_byte_offset_in_visual_text(result)
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        byte_offset: usize,
    ) -> LogicalRect {
        let window_adapter = match self.window_adapter_weak.upgrade() {
            Some(window) => window,
            None => return Default::default(),
        };

        let window = WindowInner::from_pub(window_adapter.window());

        let text = text_input.text();
        let scale_factor = ScaleFactor::new(window.scale_factor());

        let font_size =
            text_input.font_request(&window_adapter).pixel_size.unwrap_or(fonts::DEFAULT_FONT_SIZE);

        let mut result = PhysicalPoint::default();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width.get() <= 0. || height.get() <= 0. {
            return LogicalRect::new(
                LogicalPoint::default(),
                LogicalSize::from_lengths(LogicalLength::new(1.0), font_size),
            );
        }

        let font = crate::fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input.font_request(&window_adapter),
                scale_factor,
                &text_input.text(),
            )
        });

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        fonts::layout_text_lines(
            text.as_str(),
            &font,
            PhysicalSize::from_lengths(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::Clip,
            text_input.single_line(),
            &paint,
            |line_text, line_pos, start, metrics| {
                if (start..=(start + line_text.len())).contains(&byte_offset) {
                    for glyph in &metrics.glyphs {
                        if glyph.byte_index == (byte_offset - start) {
                            result = line_pos + PhysicalPoint::new(glyph.x, 0.0).to_vector();
                            return;
                        }
                    }
                    if let Some(last) = metrics.glyphs.last() {
                        if line_text.ends_with('\n') {
                            result = line_pos + euclid::vec2(0.0, last.y);
                        } else {
                            result = line_pos + euclid::vec2(last.x + last.advance_x, 0.0);
                        }
                    }
                }
            },
        );

        LogicalRect::new(
            result / scale_factor,
            LogicalSize::from_lengths(LogicalLength::new(1.0), font_size),
        )
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        fonts::register_font_from_memory(data)
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        fonts::register_font_from_path(path)
    }

    fn default_font_size(&self) -> LogicalLength {
        self::fonts::DEFAULT_FONT_SIZE
    }

    fn free_graphics_resources(
        &self,
        component: i_slint_core::component::ComponentRef,
        _items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.graphics_cache.component_destroyed(component);
        Ok(())
    }
}

impl Drop for FemtoVGRenderer {
    fn drop(&mut self) {
        // Clear these manually to drop any Rc<Canvas>.
        self.graphics_cache.clear_all();
        self.texture_cache.borrow_mut().clear();

        if Rc::strong_count(&self.canvas) != 1 {
            i_slint_core::debug_log!("internal warning: there are canvas references left when destroying the window. OpenGL resources will be leaked.")
        }
    }
}
