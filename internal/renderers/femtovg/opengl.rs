// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{cell::RefCell, num::NonZeroU32, rc::Rc};

use i_slint_core::api::PlatformError;

use crate::{FemtoVGRenderer, GraphicsBackend, WindowSurface};

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

pub struct OpenGLBackend {
    opengl_context: RefCell<Box<dyn OpenGLInterface>>,
    #[cfg(target_family = "wasm")]
    html_canvas: RefCell<Option<web_sys::HtmlCanvasElement>>,
}

impl OpenGLBackend {
    pub fn set_opengl_context(
        &self,
        renderer: &FemtoVGRenderer<Self>,
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

        let femtovg_canvas = femtovg::Canvas::new_with_text_context(
            gl_renderer,
            crate::fonts::FONT_CACHE.with(|cache| cache.borrow().text_context.clone()),
        )
        .unwrap();

        *self.opengl_context.borrow_mut() = opengl_context;
        #[cfg(target_family = "wasm")]
        {
            *self.html_canvas.borrow_mut() = Some(html_canvas);
        }

        let canvas = Rc::new(RefCell::new(femtovg_canvas));
        renderer.reset_canvas(canvas);
        Ok(())
    }
}

pub struct GLWindowSurface {}

impl WindowSurface<femtovg::renderer::OpenGl> for GLWindowSurface {
    fn render_surface(&self) -> &<femtovg::renderer::OpenGl as femtovg::Renderer>::Surface {
        &()
    }
}

impl GraphicsBackend for OpenGLBackend {
    type Renderer = femtovg::renderer::OpenGl;
    type WindowSurface = GLWindowSurface;
    const NAME: &'static str = "OpenGL";

    fn new_suspended() -> Self {
        Self {
            opengl_context: RefCell::new(Box::new(SuspendedRenderer {})),
            #[cfg(target_family = "wasm")]
            html_canvas: RefCell::new(None),
        }
    }

    fn clear_graphics_context(&self) {
        *self.opengl_context.borrow_mut() = Box::new(SuspendedRenderer {});
    }

    /// Ensures that the OpenGL context is current when returning from this function.
    fn begin_surface_rendering(
        &self,
    ) -> Result<GLWindowSurface, Box<dyn std::error::Error + Send + Sync>> {
        self.opengl_context.borrow().ensure_current()?;
        Ok(GLWindowSurface {})
    }

    fn submit_commands(&self, _commands: <Self::Renderer as femtovg::Renderer>::CommandBuffer) {}

    /// This function is called by the renderers when all OpenGL commands have been issued and
    /// the back buffer is reading for on-screen presentation. Typically implementations forward
    /// this to platform specific APIs such as eglSwapBuffers.
    fn present_surface(
        &self,
        _surface: GLWindowSurface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.opengl_context.borrow().swap_buffers()
    }

    #[cfg(not(target_family = "wasm"))]
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        use i_slint_core::api::GraphicsAPI;

        self.opengl_context.borrow().ensure_current()?;
        let api = GraphicsAPI::NativeOpenGL {
            get_proc_address: &|name| self.opengl_context.borrow().get_proc_address(name),
        };
        Ok(callback(Some(api)))
    }

    #[cfg(target_family = "wasm")]
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        use i_slint_core::api::GraphicsAPI;

        let id =
            self.html_canvas.borrow().as_ref().map_or_else(|| String::new(), |canvas| canvas.id());

        let api = GraphicsAPI::WebGL { canvas_element_id: &id, context_type: "webgl2" };
        Ok(callback(Some(api)))
    }

    /// This function is called by the renderers when the surface needs to be resized, typically
    /// in response to the windowing system notifying of a change in the window system.
    /// For most implementations this is a no-op, with the exception for wayland for example.
    fn resize(
        &self,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.opengl_context.borrow().resize(width, height)
    }
}

impl FemtoVGRenderer<OpenGLBackend> {
    /// Creates a new renderer that renders using OpenGL. An implementation of the OpenGLInterface
    /// trait needs to supplied.
    pub fn new(
        #[cfg(not(target_arch = "wasm32"))] opengl_context: impl OpenGLInterface + 'static,
        #[cfg(target_arch = "wasm32")] html_canvas: web_sys::HtmlCanvasElement,
    ) -> Result<Self, PlatformError> {
        use super::FemtoVGRendererExt;
        let this = Self::new_suspended();
        this.graphics_backend.set_opengl_context(
            &this,
            #[cfg(not(target_arch = "wasm32"))]
            opengl_context,
            #[cfg(target_arch = "wasm32")]
            html_canvas,
        )?;
        Ok(this)
    }
}
