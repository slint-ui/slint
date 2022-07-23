// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::{
    cell::RefCell,
    pin::Pin,
    rc::{Rc, Weak},
};

use i_slint_core::{
    api::euclid,
    graphics::rendering_metrics_collector::RenderingMetricsCollector,
    graphics::{Point, Rect, Size},
    Coord,
};

use self::itemrenderer::GLCanvasRc;

mod fonts;
mod images;
mod itemrenderer;

const PASSWORD_CHARACTER: &str = "●";

pub struct Renderer {
    window_weak: Weak<i_slint_core::window::WindowInner>,
}

impl Renderer {
    pub fn new(window_weak: &Weak<i_slint_core::window::WindowInner>) -> Self {
        Self { window_weak: window_weak.clone() }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn create_canvas_from_glutin_context(
        &self,
        gl_context: &glutin::WindowedContext<glutin::PossiblyCurrent>,
    ) -> Canvas {
        let _platform_window = gl_context.window();

        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let winsys = "HTML Canvas";
            } else if #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))] {
                use winit::platform::unix::WindowExtUnix;
                let mut winsys = "unknown";

                #[cfg(feature = "x11")]
                if _platform_window.xlib_window().is_some() {
                    winsys = "x11";
                }

                #[cfg(feature = "wayland")]
                if _platform_window.wayland_surface().is_some() {
                    winsys = "wayland"
                }
            } else if #[cfg(target_os = "windows")] {
                let winsys = "windows";
            } else if #[cfg(target_os = "macos")] {
                let winsys = "macos";
            } else {
                let winsys = "unknown";
            }
        }

        let rendering_metrics_collector = RenderingMetricsCollector::new(
            self.window_weak.clone(),
            &format!("GL backend (windowing system: {})", winsys),
        );

        let gl_renderer = femtovg::renderer::OpenGl::new_from_glutin_context(gl_context).unwrap();
        self.create_canvas_from_gl_renderer(gl_renderer, rendering_metrics_collector)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn create_canvas_from_html_canvas(
        &self,
        canvas_element: &web_sys::HtmlCanvasElement,
    ) -> Canvas {
        let gl_renderer = match femtovg::renderer::OpenGl::new_from_html_canvas(canvas_element) {
            Ok(gl_renderer) => gl_renderer,
            Err(_) => {
                use wasm_bindgen::JsCast;

                // I don't believe that there's a way of disabling the 2D canvas.
                let context_2d = canvas_element
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
        self.create_canvas_from_gl_renderer(gl_renderer, None)
    }

    fn create_canvas_from_gl_renderer(
        &self,
        gl_renderer: femtovg::renderer::OpenGl,
        rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,
    ) -> Canvas {
        let canvas = femtovg::Canvas::new_with_text_context(
            gl_renderer,
            self::fonts::FONT_CACHE.with(|cache| cache.borrow().text_context.clone()),
        )
        .unwrap();
        let canvas = Rc::new(RefCell::new(canvas));
        Canvas {
            canvas,
            graphics_cache: Default::default(),
            texture_cache: Default::default(),
            rendering_metrics_collector,
        }
    }

    pub fn render(
        &self,
        canvas: &Canvas,
        width: u32,
        height: u32,
        before_rendering_callback: impl FnOnce(),
    ) {
        let window = match self.window_weak.upgrade() {
            Some(window) => window,
            None => return,
        };

        window.clone().draw_contents(|components| {
            {
                let mut canvas = canvas.canvas.as_ref().borrow_mut();
                // We pass 1.0 as dpi / device pixel ratio as femtovg only uses this factor to scale
                // text metrics. Since we do the entire translation from logical pixels to physical
                // pixels on our end, we don't need femtovg to scale a second time.
                canvas.set_size(width, height, 1.0);

                if let Some(window_item) = window.window_item() {
                    canvas.clear_rect(
                        0,
                        0,
                        width,
                        height,
                        self::itemrenderer::to_femtovg_color(
                            &window_item.as_pin_ref().background(),
                        ),
                    );
                };

                // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
                // the back buffer, in order to allow the callback to provide its own rendering of the background.
                // femtovg's clear_rect() will merely schedule a clear call, so flush right away to make it immediate.
                canvas.flush();
                canvas.set_size(width, height, 1.0);
            }

            let mut item_renderer =
                self::itemrenderer::GLItemRenderer::new(canvas, &window, width, height);

            before_rendering_callback();

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
    }

    pub fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<Coord>,
    ) -> Size {
        let window = match self.window_weak.upgrade() {
            Some(window) => window,
            None => return Default::default(),
        };
        crate::renderer::femtovg::fonts::text_size(
            &font_request,
            window.scale_factor(),
            text,
            max_width,
        )
    }

    pub fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        pos: Point,
    ) -> usize {
        let window = match self.window_weak.upgrade() {
            Some(window) => window,
            None => return 0,
        };

        let scale_factor = window.scale_factor();
        let pos = pos * scale_factor;
        let text = text_input.text();

        let mut result = text.len();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width <= 0. || height <= 0. || pos.y < 0. {
            return 0;
        }

        let font = crate::renderer::femtovg::fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input.font_request(&window),
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
            Size::new(width, height),
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

    pub fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        byte_offset: usize,
    ) -> Rect {
        let window = match self.window_weak.upgrade() {
            Some(window) => window,
            None => return Default::default(),
        };

        let text = text_input.text();
        let scale_factor = window.scale_factor();

        let font_size =
            text_input.font_request(&window).pixel_size.unwrap_or(fonts::DEFAULT_FONT_SIZE);

        let mut result = Point::default();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width <= 0. || height <= 0. {
            return Rect::new(result, Size::new(1.0, font_size));
        }

        let font = crate::renderer::femtovg::fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input.font_request(&window),
                scale_factor,
                &text_input.text(),
            )
        });

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        fonts::layout_text_lines(
            text.as_str(),
            &font,
            Size::new(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::Clip,
            text_input.single_line(),
            paint,
            |line_text, line_pos, start, metrics| {
                if (start..=(start + line_text.len())).contains(&byte_offset) {
                    for glyph in &metrics.glyphs {
                        if glyph.byte_index == (byte_offset - start) {
                            result = line_pos + euclid::vec2(glyph.x, 0.0);
                            return;
                        }
                    }
                    if let Some(last) = metrics.glyphs.last() {
                        result = line_pos + euclid::vec2(last.x + last.advance_x, last.y);
                    }
                }
            },
        );

        Rect::new(result / scale_factor, Size::new(1.0, font_size))
    }

    pub fn register_font_from_memory(
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        fonts::register_font_from_memory(data)
    }

    pub fn register_font_from_path(
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        fonts::register_font_from_path(path)
    }
}

pub struct Canvas {
    canvas: GLCanvasRc,
    graphics_cache: itemrenderer::ItemGraphicsCache,
    texture_cache: RefCell<images::TextureCache>,
    rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,
}

impl Canvas {
    pub fn release_graphics_resources(&self) {
        self.graphics_cache.clear_all();
        self.texture_cache.borrow_mut().clear();
    }

    pub fn component_destroyed(&self, component: i_slint_core::component::ComponentRef) {
        self.graphics_cache.component_destroyed(component)
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        if Rc::strong_count(&self.canvas) != 1 {
            i_slint_core::debug_log!("internal warning: there are canvas references left when destroying the window. OpenGL resources will be leaked.")
        }
    }
}
