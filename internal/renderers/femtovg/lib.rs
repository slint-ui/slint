// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

use std::cell::RefCell;
use std::pin::Pin;
use std::rc::{Rc, Weak};

use i_slint_core::api::{
    PhysicalSize as PhysicalWindowSize, RenderingNotifier, RenderingState,
    SetRenderingNotifierError,
};
use i_slint_core::graphics::FontRequest;
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

/// Trait that the FemtoVGRenderer uses to ensure that the OpenGL context is current, before running
/// OpenGL commands. The trait also provides access to the symbols of the OpenGL implementation.
pub trait OpenGLContextWrapper {
    /// Ensures that the GL context is current.
    fn ensure_current(&self) -> Result<(), PlatformError>;
    fn swap_buffers(&self) -> Result<(), PlatformError>;
    fn resize(&self, size: PhysicalWindowSize) -> Result<(), PlatformError>;
    #[cfg(not(target_arch = "wasm32"))]
    fn get_proc_address(&self, name: &std::ffi::CStr) -> *const std::ffi::c_void;
    #[cfg(target_arch = "wasm32")]
    fn html_canvas_element(&self) -> web_sys::HtmlCanvasElement;
}

/// Use the FemtoVG renderer when implementing a custom Slint platform where you deliver events to
/// Slint and want the scene to be rendered using OpenGL and the FemtoVG renderer.
pub struct FemtoVGRenderer {
    window_adapter_weak: Weak<dyn WindowAdapter>,
    rendering_notifier: RefCell<Option<Box<dyn RenderingNotifier>>>,
    canvas: CanvasRc,
    graphics_cache: itemrenderer::ItemGraphicsCache,
    texture_cache: RefCell<images::TextureCache>,
    rendering_metrics_collector: RefCell<Option<Rc<RenderingMetricsCollector>>>,
    // Last field, so that it's dropped last and context exists and is current when destroying the FemtoVG canvas
    opengl_context: Box<dyn OpenGLContextWrapper>,
}

impl FemtoVGRenderer {
    /// Creates a new renderer is associated with the provided window adapter and an implementation
    /// of the OpenGLContextWrapper trait. The trait serves the purpose of giving the renderer control
    /// over when the make the context current, how to retrieve the address of GL functions, and when
    /// to swap back and front buffers.
    pub fn new(
        window_adapter_weak: &Weak<dyn WindowAdapter>,
        opengl_context: impl OpenGLContextWrapper + 'static,
    ) -> Result<Self, PlatformError> {
        let opengl_context = Box::new(opengl_context);
        #[cfg(not(target_arch = "wasm32"))]
        let gl_renderer = unsafe {
            femtovg::renderer::OpenGl::new_from_function_cstr(|name| {
                opengl_context.get_proc_address(name)
            })
            .unwrap()
        };

        #[cfg(target_arch = "wasm32")]
        let canvas = opengl_context.html_canvas_element();

        #[cfg(target_arch = "wasm32")]
        let gl_renderer = match femtovg::renderer::OpenGl::new_from_html_canvas(&canvas) {
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
            rendering_notifier: Default::default(),
            canvas,
            graphics_cache: Default::default(),
            texture_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            opengl_context,
        })
    }

    /// Notifiers the renderer that the underlying window is becoming visible.
    pub fn show(&self) -> Result<(), PlatformError> {
        self.opengl_context.ensure_current()?;
        *self.rendering_metrics_collector.borrow_mut() =
            RenderingMetricsCollector::new(&format!("FemtoVG renderer"));

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            self.with_graphics_api(|api| callback.notify(RenderingState::RenderingSetup, &api))?;
        }

        Ok(())
    }

    /// Notifiers the renderer that the underlying window will be hidden soon.
    pub fn hide(&self) -> Result<(), PlatformError> {
        self.opengl_context.ensure_current()?;
        self.rendering_metrics_collector.borrow_mut().take();

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            self.with_graphics_api(|api| callback.notify(RenderingState::RenderingTeardown, &api))?;
        }

        Ok(())
    }

    /// Render the scene using OpenGL. This function assumes that the context is current.
    pub fn render(
        &self,
        size: PhysicalWindowSize,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.opengl_context.ensure_current()?;

        let width = size.width;
        let height = size.height;

        let window_adapter = self.window_adapter_weak.upgrade().unwrap();
        let window = WindowInner::from_pub(window_adapter.window());
        let scale = window.scale_factor().ceil();

        window.draw_contents(|components| -> Result<(), PlatformError> {
            let window_background_brush = window.window_item().map(|w| w.as_pin_ref().background());

            {
                let mut femtovg_canvas = self.canvas.borrow_mut();
                // We pass an integer that is greater than or equal to the scale factor as
                // dpi / device pixel ratio as the anti-alias of femtovg needs that to draw text clearly.
                // We need to care about that `ceil()` when calculating metrics.
                femtovg_canvas.set_size(width, height, scale);

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

            if let Some(notifier_fn) = self.rendering_notifier.borrow_mut().as_mut() {
                let mut femtovg_canvas = self.canvas.borrow_mut();
                // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
                // the back buffer, in order to allow the callback to provide its own rendering of the background.
                // femtovg's clear_rect() will merely schedule a clear call, so flush right away to make it immediate.

                femtovg_canvas.flush();

                femtovg_canvas.set_size(width, height, scale);
                drop(femtovg_canvas);

                self.with_graphics_api(|api| {
                    notifier_fn.notify(RenderingState::BeforeRendering, &api)
                })?;
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
        })?;

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            self.with_graphics_api(|api| callback.notify(RenderingState::AfterRendering, &api))?;
        }

        self.opengl_context.swap_buffers()
    }

    /// Inform the renderer about the new size of the underlying window.
    pub fn resize(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        self.opengl_context.resize(size)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn with_graphics_api(
        &self,
        callback: impl FnOnce(i_slint_core::api::GraphicsAPI<'_>),
    ) -> Result<(), PlatformError> {
        use i_slint_core::api::GraphicsAPI;

        self.opengl_context.ensure_current()?;
        let api = GraphicsAPI::NativeOpenGL {
            get_proc_address: &|name| self.opengl_context.get_proc_address(name),
        };
        callback(api);
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn with_graphics_api(
        &self,
        callback: impl FnOnce(i_slint_core::api::GraphicsAPI<'_>),
    ) -> Result<(), PlatformError> {
        use i_slint_core::api::GraphicsAPI;

        let canvas_element_id = self.opengl_context.html_canvas_element().id();
        let api = GraphicsAPI::WebGL {
            canvas_element_id: canvas_element_id.as_str(),
            context_type: "webgl",
        };
        callback(api);
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn html_canvas_element(&self) -> web_sys::HtmlCanvasElement {
        self.opengl_context.html_canvas_element()
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
        font_request: FontRequest,
        scale_factor: ScaleFactor,
    ) -> usize {
        let pos = pos * scale_factor;
        let text = text_input.text();

        let mut result = text.len();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width.get() <= 0. || height.get() <= 0. || pos.y < 0. {
            return 0;
        }

        let font = crate::fonts::FONT_CACHE
            .with(|cache| cache.borrow_mut().font(font_request, scale_factor, &text_input.text()));

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
        font_request: FontRequest,
        scale_factor: ScaleFactor,
    ) -> LogicalRect {
        let text = text_input.text();

        let font_size = font_request.pixel_size.unwrap_or(fonts::DEFAULT_FONT_SIZE);

        let mut result = PhysicalPoint::default();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width.get() <= 0. || height.get() <= 0. {
            return LogicalRect::new(
                LogicalPoint::default(),
                LogicalSize::from_lengths(LogicalLength::new(1.0), font_size),
            );
        }

        let font = crate::fonts::FONT_CACHE
            .with(|cache| cache.borrow_mut().font(font_request, scale_factor, &text_input.text()));

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

    fn set_rendering_notifier(
        &self,
        callback: Box<dyn i_slint_core::api::RenderingNotifier>,
    ) -> Result<(), i_slint_core::api::SetRenderingNotifierError> {
        let mut notifier = self.rendering_notifier.borrow_mut();
        if notifier.replace(callback).is_some() {
            Err(SetRenderingNotifierError::AlreadySet)
        } else {
            Ok(())
        }
    }

    fn free_graphics_resources(
        &self,
        component: i_slint_core::component::ComponentRef,
        _items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.opengl_context.ensure_current()?;
        self.graphics_cache.component_destroyed(component);
        Ok(())
    }
}

impl Drop for FemtoVGRenderer {
    fn drop(&mut self) {
        // Ensure the context is current before the renderer is destroyed
        self.opengl_context.ensure_current().ok();

        // Clear these manually to drop any Rc<Canvas>.
        self.graphics_cache.clear_all();
        self.texture_cache.borrow_mut().clear();

        if Rc::strong_count(&self.canvas) != 1 {
            i_slint_core::debug_log!("internal warning: there are canvas references left when destroying the window. OpenGL resources will be leaked.")
        }
    }
}
