// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;
use std::pin::Pin;
use std::rc::{Rc, Weak};

use i_slint_common::sharedfontdb;
use i_slint_core::api::{RenderingNotifier, RenderingState, SetRenderingNotifierError};
use i_slint_core::graphics::{euclid, rendering_metrics_collector::RenderingMetricsCollector};
use i_slint_core::graphics::{BorderRadius, Rgba8Pixel};
use i_slint_core::graphics::{FontRequest, SharedPixelBuffer};
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::items::TextWrap;
use i_slint_core::lengths::{
    LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PhysicalPx, ScaleFactor,
};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::RendererSealed;
use i_slint_core::window::{WindowAdapter, WindowInner};
use i_slint_core::Brush;

type PhysicalLength = euclid::Length<f32, PhysicalPx>;
type PhysicalRect = euclid::Rect<f32, PhysicalPx>;
type PhysicalSize = euclid::Size2D<f32, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<f32, PhysicalPx>;
type PhysicalBorderRadius = BorderRadius<f32, PhysicalPx>;

use self::itemrenderer::CanvasRc;

mod fonts;
mod images;
mod itemrenderer;

/// This trait describes the interface GPU accelerated renderers in Slint require to render with OpenGL.
///
/// It serves the purpose to ensure that the OpenGL context is current before running any OpenGL
/// commands, as well as providing access to the OpenGL implementation by function pointers.
///
/// # Safety
///
/// This trait is unsafe because an implementation of get_proc_address could return dangling
/// pointers. In practice an implementation of this trait should just forward to the EGL/WGL/CGL
/// C library that implements EGL/CGL/WGL.
#[allow(unsafe_code)]
pub unsafe trait OpenGLInterface {
    /// Ensures that the OpenGL context is current when returning from this function.
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// This function is called by the renderers when all OpenGL commands have been issued and
    /// the back buffer is reading for on-screen presentation. Typically implementations forward
    /// this to platform specific APIs such as eglSwapBuffers.
    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// This function is called by the renderers when the surface needs to be resized, typically
    /// in response to the windowing system notifying of a change in the window system.
    /// For most implementations this is a no-op, with the exception for wayland for example.
    fn resize(
        &self,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Returns the address of the OpenGL function specified by name, or a null pointer if the
    /// function does not exist.
    fn get_proc_address(&self, name: &std::ffi::CStr) -> *const std::ffi::c_void;
}

#[cfg(target_arch = "wasm32")]
struct WebGLNeedsNoCurrentContext;
#[cfg(target_arch = "wasm32")]
unsafe impl OpenGLInterface for WebGLNeedsNoCurrentContext {
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn resize(
        &self,
        _width: NonZeroU32,
        _height: NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn get_proc_address(&self, _: &std::ffi::CStr) -> *const std::ffi::c_void {
        unreachable!()
    }
}

struct SuspendedRenderer {}

unsafe impl OpenGLInterface for SuspendedRenderer {
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("ensure current called on suspended renderer".to_string().into())
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("swap_buffers called on suspended renderer".to_string().into())
    }

    fn resize(
        &self,
        _: NonZeroU32,
        _: NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn get_proc_address(&self, _: &std::ffi::CStr) -> *const std::ffi::c_void {
        panic!("get_proc_address called on suspended renderer")
    }
}

/// Use the FemtoVG renderer when implementing a custom Slint platform where you deliver events to
/// Slint and want the scene to be rendered using OpenGL. The rendering is done using the [FemtoVG](https://github.com/femtovg/femtovg)
/// library.
pub struct FemtoVGRenderer {
    maybe_window_adapter: RefCell<Option<Weak<dyn WindowAdapter>>>,
    rendering_notifier: RefCell<Option<Box<dyn RenderingNotifier>>>,
    canvas: RefCell<Option<CanvasRc>>,
    graphics_cache: itemrenderer::ItemGraphicsCache,
    texture_cache: RefCell<images::TextureCache>,
    rendering_metrics_collector: RefCell<Option<Rc<RenderingMetricsCollector>>>,
    rendering_first_time: Cell<bool>,
    // Last field, so that it's dropped last and context exists and is current when destroying the FemtoVG canvas
    opengl_context: RefCell<Box<dyn OpenGLInterface>>,
    #[cfg(target_arch = "wasm32")]
    canvas_id: RefCell<String>,
}

impl FemtoVGRenderer {
    /// Creates a new renderer that renders using OpenGL. An implementation of the OpenGLInterface
    /// trait needs to supplied.
    pub fn new(
        #[cfg(not(target_arch = "wasm32"))] opengl_context: impl OpenGLInterface + 'static,
        #[cfg(target_arch = "wasm32")] html_canvas: web_sys::HtmlCanvasElement,
    ) -> Result<Self, PlatformError> {
        let this = Self::new_without_context();
        this.set_opengl_context(
            #[cfg(not(target_arch = "wasm32"))]
            opengl_context,
            #[cfg(target_arch = "wasm32")]
            html_canvas,
        )?;
        Ok(this)
    }

    /// Render the scene using OpenGL.
    pub fn render(&self) -> Result<(), i_slint_core::platform::PlatformError> {
        self.internal_render_with_post_callback(
            0.,
            (0., 0.),
            self.window_adapter()?.window().size(),
            None,
        )
    }

    fn internal_render_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: i_slint_core::api::PhysicalSize,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.opengl_context.borrow().ensure_current()?;

        if self.rendering_first_time.take() {
            *self.rendering_metrics_collector.borrow_mut() =
                RenderingMetricsCollector::new("FemtoVG renderer");

            if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                self.with_graphics_api(|api| {
                    callback.notify(RenderingState::RenderingSetup, &api)
                })?;
            }
        }

        let window_adapter = self.window_adapter()?;
        let window = window_adapter.window();
        let window_size = window.size();

        let Some((width, height)): Option<(NonZeroU32, NonZeroU32)> =
            window_size.width.try_into().ok().zip(window_size.height.try_into().ok())
        else {
            // Nothing to render
            return Ok(());
        };

        if self.canvas.borrow().is_none() {
            // Nothing to render
            return Ok(());
        }

        let window_inner = WindowInner::from_pub(window);
        let scale = window_inner.scale_factor().ceil();

        window_inner
            .draw_contents(|components| -> Result<(), PlatformError> {
                // self.canvas is checked for being Some(...) at the beginning of this function
                let canvas = self.canvas.borrow().as_ref().unwrap().clone();

                let window_background_brush =
                    window_inner.window_item().map(|w| w.as_pin_ref().background());

                {
                    let mut femtovg_canvas = canvas.borrow_mut();
                    // We pass an integer that is greater than or equal to the scale factor as
                    // dpi / device pixel ratio as the anti-alias of femtovg needs that to draw text clearly.
                    // We need to care about that `ceil()` when calculating metrics.
                    femtovg_canvas.set_size(surface_size.width, surface_size.height, scale);

                    // Clear with window background if it is a solid color otherwise it will drawn as gradient
                    if let Some(Brush::SolidColor(clear_color)) = window_background_brush {
                        femtovg_canvas.clear_rect(
                            0,
                            0,
                            surface_size.width,
                            surface_size.height,
                            self::itemrenderer::to_femtovg_color(&clear_color),
                        );
                    }
                }

                {
                    let mut femtovg_canvas = canvas.borrow_mut();
                    femtovg_canvas.reset();
                    femtovg_canvas.rotate(rotation_angle_degrees.to_radians());
                    femtovg_canvas.translate(translation.0, translation.1);
                }

                if let Some(notifier_fn) = self.rendering_notifier.borrow_mut().as_mut() {
                    let mut femtovg_canvas = canvas.borrow_mut();
                    // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
                    // the back buffer, in order to allow the callback to provide its own rendering of the background.
                    // femtovg's clear_rect() will merely schedule a clear call, so flush right away to make it immediate.

                    femtovg_canvas.flush();

                    femtovg_canvas.set_size(width.get(), height.get(), scale);
                    drop(femtovg_canvas);

                    self.with_graphics_api(|api| {
                        notifier_fn.notify(RenderingState::BeforeRendering, &api)
                    })?;
                }

                self.graphics_cache.clear_cache_if_scale_factor_changed(window);

                let mut item_renderer = self::itemrenderer::GLItemRenderer::new(
                    &canvas,
                    &self.graphics_cache,
                    &self.texture_cache,
                    window,
                    width.get(),
                    height.get(),
                );

                if let Some(window_item_rc) = window_inner.window_item_rc() {
                    let window_item =
                        window_item_rc.downcast::<i_slint_core::items::WindowItem>().unwrap();
                    match window_item.as_pin_ref().background() {
                        Brush::SolidColor(..) => {
                            // clear_rect is called earlier
                        }
                        _ => {
                            // Draws the window background as gradient
                            item_renderer.draw_rectangle(
                                window_item.as_pin_ref(),
                                &window_item_rc,
                                i_slint_core::lengths::logical_size_from_api(
                                    window.size().to_logical(window_inner.scale_factor()),
                                ),
                                &window_item.as_pin_ref().cached_rendering_data,
                            );
                        }
                    }
                }

                for (component, origin) in components {
                    i_slint_core::item_rendering::render_component_items(
                        component,
                        &mut item_renderer,
                        *origin,
                        &self.window_adapter()?,
                    );
                }

                if let Some(cb) = post_render_cb.as_ref() {
                    cb(&mut item_renderer)
                }

                if let Some(collector) = &self.rendering_metrics_collector.borrow().as_ref() {
                    collector.measure_frame_rendered(&mut item_renderer);
                }

                canvas.borrow_mut().flush();

                // Delete any images and layer images (and their FBOs) before making the context not current anymore, to
                // avoid GPU memory leaks.
                self.texture_cache.borrow_mut().drain();
                drop(item_renderer);
                Ok(())
            })
            .unwrap_or(Ok(()))?;

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            self.with_graphics_api(|api| callback.notify(RenderingState::AfterRendering, &api))?;
        }

        self.opengl_context.borrow().swap_buffers()?;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn with_graphics_api(
        &self,
        callback: impl FnOnce(i_slint_core::api::GraphicsAPI<'_>),
    ) -> Result<(), PlatformError> {
        use i_slint_core::api::GraphicsAPI;

        self.opengl_context.borrow().ensure_current()?;
        let api = GraphicsAPI::NativeOpenGL {
            get_proc_address: &|name| self.opengl_context.borrow().get_proc_address(name),
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

        let canvas_id = self.canvas_id.borrow();

        let api =
            GraphicsAPI::WebGL { canvas_element_id: canvas_id.as_str(), context_type: "webgl2" };
        callback(api);
        Ok(())
    }

    fn window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade()).ok_or_else(|| {
            "Renderer must be associated with component before use".to_string().into()
        })
    }
}

#[doc(hidden)]
impl RendererSealed for FemtoVGRenderer {
    fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<LogicalLength>,
        scale_factor: ScaleFactor,
        _text_wrap: TextWrap, //TODO: Add support for char-wrap
    ) -> LogicalSize {
        crate::fonts::text_size(&font_request, scale_factor, text, max_width)
    }

    fn font_metrics(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        _scale_factor: ScaleFactor,
    ) -> i_slint_core::items::FontMetrics {
        crate::fonts::font_metrics(font_request)
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
            None,
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
        let cursor_point = fonts::layout_text_lines(
            text.as_str(),
            &font,
            PhysicalSize::from_lengths(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::Clip,
            text_input.single_line(),
            Some(byte_offset),
            &paint,
            |_, _, _, _| {},
        );

        LogicalRect::new(
            cursor_point.unwrap_or_default() / scale_factor,
            LogicalSize::from_lengths(LogicalLength::new(1.0), font_size),
        )
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        sharedfontdb::register_font_from_memory(data)
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sharedfontdb::register_font_from_path(path)
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
        component: i_slint_core::item_tree::ItemTreeRef,
        _items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        if !self.graphics_cache.is_empty() {
            self.opengl_context.borrow().ensure_current()?;
            self.graphics_cache.component_destroyed(component);
        }
        Ok(())
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        *self.maybe_window_adapter.borrow_mut() = Some(Rc::downgrade(window_adapter));
        if self.opengl_context.borrow().ensure_current().is_ok() {
            self.graphics_cache.clear_all();
            self.texture_cache.borrow_mut().clear();
        }
    }

    fn resize(&self, size: i_slint_core::api::PhysicalSize) -> Result<(), PlatformError> {
        if let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok()) {
            self.opengl_context.borrow().resize(width, height)?;
        };
        Ok(())
    }

    /// Returns an image buffer of what was rendered last by reading the previous front buffer (using glReadPixels).
    fn take_snapshot(&self) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError> {
        self.opengl_context.borrow().ensure_current()?;
        let Some(canvas) = self.canvas.borrow().as_ref().cloned() else {
            return Err("FemtoVG renderer cannot take screenshot without a window".into());
        };
        let screenshot = canvas
            .borrow_mut()
            .screenshot()
            .map_err(|e| format!("FemtoVG error reading current back buffer: {e}"))?;

        use rgb::ComponentBytes;
        Ok(SharedPixelBuffer::clone_from_slice(
            screenshot.buf().as_bytes(),
            screenshot.width() as u32,
            screenshot.height() as u32,
        ))
    }
}

impl Drop for FemtoVGRenderer {
    fn drop(&mut self) {
        self.clear_opengl_context().ok();
    }
}

#[doc(hidden)]
pub trait FemtoVGRendererExt {
    fn new_without_context() -> Self;
    fn set_opengl_context(
        &self,
        #[cfg(not(target_arch = "wasm32"))] opengl_context: impl OpenGLInterface + 'static,
        #[cfg(target_arch = "wasm32")] html_canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(), i_slint_core::platform::PlatformError>;
    fn clear_opengl_context(&self) -> Result<(), i_slint_core::platform::PlatformError>;
    fn render_transformed_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: i_slint_core::api::PhysicalSize,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Result<(), i_slint_core::platform::PlatformError>;
}

#[doc(hidden)]
impl FemtoVGRendererExt for FemtoVGRenderer {
    /// Creates a new renderer in suspended state without OpenGL. Any attempts at rendering, etc. will produce an error,
    /// until [`Self::set_opengl_context()`] was called successfully.
    fn new_without_context() -> Self {
        let opengl_context = Box::new(SuspendedRenderer {});

        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            canvas: RefCell::new(None),
            graphics_cache: Default::default(),
            texture_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Cell::new(true),
            opengl_context: RefCell::new(opengl_context),
            #[cfg(target_arch = "wasm32")]
            canvas_id: Default::default(),
        }
    }

    fn clear_opengl_context(&self) -> Result<(), i_slint_core::platform::PlatformError> {
        // Ensure the context is current before the renderer is destroyed
        if self.opengl_context.borrow().ensure_current().is_ok() {
            // If we've rendered a frame before, then we need to invoke the RenderingTearDown notifier.
            if !self.rendering_first_time.get() {
                if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                    self.with_graphics_api(|api| {
                        callback.notify(RenderingState::RenderingTeardown, &api)
                    })
                    .ok();
                }
            }

            self.graphics_cache.clear_all();
            self.texture_cache.borrow_mut().clear();
        }

        if let Some(canvas) = self.canvas.borrow_mut().take() {
            if Rc::strong_count(&canvas) != 1 {
                i_slint_core::debug_log!("internal warning: there are canvas references left when destroying the window. OpenGL resources will be leaked.")
            }
        }

        *self.opengl_context.borrow_mut() = Box::new(SuspendedRenderer {});

        Ok(())
    }

    fn set_opengl_context(
        &self,
        #[cfg(not(target_arch = "wasm32"))] opengl_context: impl OpenGLInterface + 'static,
        #[cfg(target_arch = "wasm32")] html_canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        #[cfg(target_arch = "wasm32")]
        let opengl_context = WebGLNeedsNoCurrentContext {};

        let opengl_context = Box::new(opengl_context);
        #[cfg(not(target_arch = "wasm32"))]
        let gl_renderer = unsafe {
            femtovg::renderer::OpenGl::new_from_function_cstr(|name| {
                opengl_context.get_proc_address(name)
            })
            .unwrap()
        };

        #[cfg(target_arch = "wasm32")]
        let gl_renderer = match femtovg::renderer::OpenGl::new_from_html_canvas(&html_canvas) {
            Ok(gl_renderer) => gl_renderer,
            Err(_) => {
                use wasm_bindgen::JsCast;

                // I don't believe that there's a way of disabling the 2D canvas.
                let context_2d = html_canvas
                    .get_context("2d")
                    .unwrap()
                    .unwrap()
                    .dyn_into::<web_sys::CanvasRenderingContext2d>()
                    .unwrap();
                context_2d.set_font("20px serif");
                // We don't know if we're rendering on dark or white background, so choose a "color" in the middle for the text.
                context_2d.set_fill_style_str("red");
                context_2d
                    .fill_text("Slint requires WebGL to be enabled in your browser", 0., 30.)
                    .unwrap();
                panic!("Cannot proceed without WebGL - aborting")
            }
        };

        #[cfg(target_arch = "wasm32")]
        {
            *self.canvas_id.borrow_mut() = html_canvas.id();
        }

        let femtovg_canvas = femtovg::Canvas::new_with_text_context(
            gl_renderer,
            self::fonts::FONT_CACHE.with(|cache| cache.borrow().text_context.clone()),
        )
        .unwrap();
        let canvas = Rc::new(RefCell::new(femtovg_canvas));

        *self.canvas.borrow_mut() = canvas.into();
        *self.opengl_context.borrow_mut() = opengl_context;
        self.rendering_first_time.set(true);
        Ok(())
    }

    fn render_transformed_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: i_slint_core::api::PhysicalSize,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.internal_render_with_post_callback(
            rotation_angle_degrees,
            translation,
            surface_size,
            post_render_cb,
        )
    }
}
