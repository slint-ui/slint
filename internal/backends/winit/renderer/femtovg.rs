// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::RefCell;
use std::pin::Pin;
use std::rc::{Rc, Weak};

use i_slint_core::api::{
    GraphicsAPI, PhysicalSize as PhysicalWindowSize, RenderingNotifier, RenderingState,
    SetRenderingNotifierError,
};
use i_slint_core::graphics::{euclid, rendering_metrics_collector::RenderingMetricsCollector};
use i_slint_core::items::Item;
use i_slint_core::lengths::{
    LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PhysicalPx, ScaleFactor,
};
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

const PASSWORD_CHARACTER: &str = "●";

pub struct FemtoVGRenderer {
    window_adapter_weak: Weak<dyn WindowAdapter>,
    rendering_notifier: RefCell<Option<Box<dyn RenderingNotifier>>>,
    canvas: RefCell<Option<FemtoVGCanvas>>,
}

impl super::WinitCompatibleRenderer for FemtoVGRenderer {
    const NAME: &'static str = "FemtoVG";

    fn new(window_adapter_weak: &Weak<dyn WindowAdapter>) -> Self {
        Self {
            window_adapter_weak: window_adapter_weak.clone(),
            rendering_notifier: Default::default(),
            canvas: Default::default(),
        }
    }

    fn show(
        &self,
        window: &Rc<winit::window::Window>,
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) {
        let size: winit::dpi::PhysicalSize<u32> = window.inner_size();
        let opengl_context = crate::OpenGLContext::new_context(
            window,
            window,
            PhysicalWindowSize::new(size.width, size.height),
            #[cfg(target_arch = "wasm32")]
            canvas_id,
        );

        let rendering_metrics_collector = RenderingMetricsCollector::new(
            self.window_adapter_weak.clone(),
            &format!("FemtoVG renderer (windowing system: {})", crate::winsys_name(window)),
        );

        #[cfg(not(target_arch = "wasm32"))]
        let gl_renderer = unsafe {
            femtovg::renderer::OpenGl::new_from_function(|s| {
                opengl_context.get_proc_address(s) as *const _
            })
            .unwrap()
        };

        #[cfg(target_arch = "wasm32")]
        let gl_renderer = match femtovg::renderer::OpenGl::new_from_html_canvas(
            &opengl_context.html_canvas_element(),
        ) {
            Ok(gl_renderer) => gl_renderer,
            Err(_) => {
                use wasm_bindgen::JsCast;

                // I don't believe that there's a way of disabling the 2D canvas.
                let context_2d = opengl_context
                    .html_canvas_element()
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
        let femtovg_canvas = Rc::new(RefCell::new(femtovg_canvas));
        let canvas = FemtoVGCanvas {
            canvas: femtovg_canvas,
            graphics_cache: Default::default(),
            texture_cache: Default::default(),
            rendering_metrics_collector,
            opengl_context,
        };

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            canvas.with_graphics_api(|api| callback.notify(RenderingState::RenderingSetup, &api))
        }

        *self.canvas.borrow_mut() = Some(canvas);
    }

    fn hide(&self) {
        if let Some(canvas) = self.canvas.borrow_mut().take() {
            canvas.opengl_context.with_current_context(|_| {
                if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                    canvas.with_graphics_api(|api| {
                        callback.notify(RenderingState::RenderingTeardown, &api)
                    })
                }
            })
        }
    }

    fn render(&self, size: PhysicalWindowSize) {
        let width = size.width;
        let height = size.height;

        let canvas = if self.canvas.borrow().is_some() {
            std::cell::Ref::map(self.canvas.borrow(), |canvas_opt| canvas_opt.as_ref().unwrap())
        } else {
            return;
        };

        canvas.opengl_context.make_current();

        let window_adapter = self.window_adapter_weak.upgrade().unwrap();
        let window = WindowInner::from_pub(window_adapter.window());

        window.draw_contents(|components| {
            let window_background_brush = window.window_item().map(|w| w.as_pin_ref().background());

            {
                let mut femtovg_canvas = canvas.canvas.as_ref().borrow_mut();
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

            if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                let mut femtovg_canvas = canvas.canvas.as_ref().borrow_mut();
                // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
                // the back buffer, in order to allow the callback to provide its own rendering of the background.
                // femtovg's clear_rect() will merely schedule a clear call, so flush right away to make it immediate.

                femtovg_canvas.flush();

                femtovg_canvas.set_size(width, height, 1.0);
                drop(femtovg_canvas);

                canvas
                    .with_graphics_api(|api| callback.notify(RenderingState::BeforeRendering, &api))
            }

            let window_adapter = self.window_adapter_weak.upgrade().unwrap();

            let mut item_renderer = self::itemrenderer::GLItemRenderer::new(
                &canvas,
                window_adapter.window(),
                width,
                height,
            );

            // Draws the window background as gradient
            match window_background_brush {
                Some(Brush::SolidColor(..)) | None => {}
                Some(brush @ _) => {
                    if let Some(window_item) = window.window_item() {
                        item_renderer.draw_rect(window_item.as_pin_ref().geometry(), brush);
                    }
                }
            }

            for (component, origin) in components {
                i_slint_core::item_rendering::render_component_items(
                    component,
                    &mut item_renderer,
                    *origin,
                );
            }

            if let Some(collector) = &canvas.rendering_metrics_collector {
                collector.measure_frame_rendered(&mut item_renderer);
            }

            canvas.canvas.borrow_mut().flush();

            // Delete any images and layer images (and their FBOs) before making the context not current anymore, to
            // avoid GPU memory leaks.
            canvas.texture_cache.borrow_mut().drain();
            drop(item_renderer);
        });

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            canvas.with_graphics_api(|api| callback.notify(RenderingState::AfterRendering, &api))
        }

        canvas.opengl_context.swap_buffers();
        canvas.opengl_context.make_not_current();
    }

    fn default_font_size() -> LogicalLength {
        self::fonts::DEFAULT_FONT_SIZE
    }

    fn as_core_renderer(&self) -> &dyn Renderer {
        self
    }

    fn component_destroyed(&self, component: i_slint_core::component::ComponentRef) {
        let canvas = if self.canvas.borrow().is_some() {
            std::cell::Ref::map(self.canvas.borrow(), |canvas_opt| canvas_opt.as_ref().unwrap())
        } else {
            return;
        };

        canvas
            .opengl_context
            .with_current_context(|_| canvas.graphics_cache.component_destroyed(component))
    }

    fn resize_event(&self, size: PhysicalWindowSize) {
        let canvas = if self.canvas.borrow().is_some() {
            std::cell::Ref::map(self.canvas.borrow(), |canvas_opt| canvas_opt.as_ref().unwrap())
        } else {
            return;
        };

        canvas.opengl_context.ensure_resized(size)
    }

    #[cfg(target_arch = "wasm32")]
    fn html_canvas_element(&self) -> web_sys::HtmlCanvasElement {
        self.canvas.borrow().as_ref().unwrap().opengl_context.html_canvas_element()
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
        crate::renderer::femtovg::fonts::text_size(&font_request, scale_factor, text, max_width)
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

        let font = crate::renderer::femtovg::fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input.font_request(&window_adapter),
                scale_factor,
                &text_input.text(),
            )
        });

        let is_password =
            matches!(text_input.input_type(), i_slint_core::items::InputType::Password);
        let password_string;
        let actual_text = if is_password {
            password_string = PASSWORD_CHARACTER.repeat(text.chars().count());
            password_string.as_str()
        } else {
            text.as_str()
        };

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        let text_context = crate::renderer::femtovg::fonts::FONT_CACHE
            .with(|cache| cache.borrow().text_context.clone());
        let font_height = text_context.measure_font(paint).unwrap().height();
        crate::renderer::femtovg::fonts::layout_text_lines(
            actual_text,
            &font,
            PhysicalSize::from_lengths(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::Clip,
            text_input.single_line(),
            paint,
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

        if is_password {
            text.char_indices()
                .nth(result / PASSWORD_CHARACTER.len())
                .map_or(text.len(), |(r, _)| r)
        } else {
            result
        }
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

        let font = crate::renderer::femtovg::fonts::FONT_CACHE.with(|cache| {
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
            paint,
            |line_text, line_pos, start, metrics| {
                if (start..=(start + line_text.len())).contains(&byte_offset) {
                    for glyph in &metrics.glyphs {
                        if glyph.byte_index == (byte_offset - start) {
                            result = line_pos + PhysicalPoint::new(glyph.x, 0.0).to_vector();
                            return;
                        }
                    }
                    if let Some(last) = metrics.glyphs.last() {
                        result = line_pos
                            + PhysicalPoint::new(last.x + last.advance_x, last.y).to_vector();
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

    fn set_rendering_notifier(
        &self,
        callback: Box<dyn RenderingNotifier>,
    ) -> std::result::Result<(), SetRenderingNotifierError> {
        let mut notifier = self.rendering_notifier.borrow_mut();
        if notifier.replace(callback).is_some() {
            Err(SetRenderingNotifierError::AlreadySet)
        } else {
            Ok(())
        }
    }
}

struct FemtoVGCanvas {
    canvas: CanvasRc,
    graphics_cache: itemrenderer::ItemGraphicsCache,
    texture_cache: RefCell<images::TextureCache>,
    rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,
    opengl_context: crate::OpenGLContext,
}

impl FemtoVGCanvas {
    #[cfg(not(target_arch = "wasm32"))]
    fn with_graphics_api(&self, callback: impl FnOnce(i_slint_core::api::GraphicsAPI<'_>)) {
        let api = GraphicsAPI::NativeOpenGL {
            get_proc_address: &|name| self.opengl_context.get_proc_address(name),
        };
        callback(api)
    }

    #[cfg(target_arch = "wasm32")]
    fn with_graphics_api(&self, callback: impl FnOnce(i_slint_core::api::GraphicsAPI<'_>)) {
        let canvas_element_id = self.opengl_context.html_canvas_element().id();
        let api = GraphicsAPI::WebGL {
            canvas_element_id: canvas_element_id.as_str(),
            context_type: "webgl",
        };
        callback(api)
    }
}

impl Drop for FemtoVGCanvas {
    fn drop(&mut self) {
        // The GL renderer must be destructed with a GL context current, in order to clean up correctly.
        self.opengl_context.make_current();

        // Clear these manually to drop any Rc<Canvas>.
        self.graphics_cache.clear_all();
        self.texture_cache.borrow_mut().clear();

        if Rc::strong_count(&self.canvas) != 1 {
            i_slint_core::debug_log!("internal warning: there are canvas references left when destroying the window. OpenGL resources will be leaked.")
        }
    }
}
