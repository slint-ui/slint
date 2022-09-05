// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::RefCell;

use i_slint_core::api::GraphicsAPI;

pub struct OpenGLSurface {
    fb_info: skia_safe::gpu::gl::FramebufferInfo,
    surface: RefCell<skia_safe::Surface>,
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
    opengl_context: crate::OpenGLContext,
}

impl super::Surface for OpenGLSurface {
    const SUPPORTS_GRAPHICS_API: bool = true;

    fn new(window_builder: winit::window::WindowBuilder) -> Self {
        let opengl_context = crate::OpenGLContext::new_context(window_builder);

        let (fb_info, surface, gr_context) =
            opengl_context.with_current_context(|opengl_context| {
                let fb_info = {
                    use glow::HasContext;

                    let gl = unsafe {
                        glow::Context::from_loader_function(|s| {
                            opengl_context.get_proc_address(s) as *const _
                        })
                    };
                    let fboid = unsafe { gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING) };

                    skia_safe::gpu::gl::FramebufferInfo {
                        fboid: fboid.try_into().unwrap(),
                        format: skia_safe::gpu::gl::Format::RGBA8.into(),
                    }
                };

                let gl_interface = skia_safe::gpu::gl::Interface::new_load_with(|symbol| {
                    opengl_context.get_proc_address(symbol)
                });

                let mut gr_context =
                    skia_safe::gpu::DirectContext::new_gl(gl_interface, None).unwrap();

                let surface = Self::create_internal_surface(
                    fb_info,
                    &opengl_context.glutin_context(),
                    &mut gr_context,
                )
                .into();

                (fb_info, surface, gr_context)
            });
        Self { fb_info, surface, gr_context: RefCell::new(gr_context), opengl_context }
    }

    fn name(&self) -> &'static str {
        "opengl"
    }

    fn with_graphics_api(&self, callback: impl FnOnce(GraphicsAPI<'_>)) {
        let api = GraphicsAPI::NativeOpenGL {
            get_proc_address: &|name| self.opengl_context.get_proc_address(name),
        };
        callback(api)
    }

    fn with_window_handle<T>(&self, callback: impl FnOnce(&winit::window::Window) -> T) -> T {
        callback(&*self.opengl_context.window())
    }

    fn with_active_surface<T>(&self, callback: impl FnOnce() -> T) -> T {
        self.opengl_context.with_current_context(|_| callback())
    }

    fn render(
        &self,
        callback: impl FnOnce(&mut skia_safe::Canvas, &mut skia_safe::gpu::DirectContext),
    ) {
        let size = self.opengl_context.window().inner_size();
        let width = size.width;
        let height = size.height;

        self.opengl_context.make_current();

        let gr_context = &mut self.gr_context.borrow_mut();

        let mut surface = self.surface.borrow_mut();
        if width != surface.width() as u32 || height != surface.height() as u32 {
            *surface = Self::create_internal_surface(
                self.fb_info,
                &self.opengl_context.glutin_context(),
                gr_context,
            );
        }

        let skia_canvas = surface.canvas();

        callback(skia_canvas, gr_context);

        self.opengl_context.swap_buffers();
        self.opengl_context.make_not_current();
    }

    fn resize_event(&self) {
        self.opengl_context.ensure_resized();
    }

    fn bits_per_pixel(&self) -> u8 {
        let pixel_format = self.opengl_context.glutin_context().get_pixel_format();
        pixel_format.color_bits + pixel_format.alpha_bits
    }
}

impl OpenGLSurface {
    fn create_internal_surface(
        fb_info: skia_safe::gpu::gl::FramebufferInfo,
        gl_context: &glutin::WindowedContext<glutin::PossiblyCurrent>,
        gr_context: &mut skia_safe::gpu::DirectContext,
    ) -> skia_safe::Surface {
        let pixel_format = gl_context.get_pixel_format();
        let size = gl_context.window().inner_size();
        let backend_render_target = skia_safe::gpu::BackendRenderTarget::new_gl(
            (size.width.try_into().unwrap(), size.height.try_into().unwrap()),
            pixel_format.multisampling.map(|s| s.try_into().unwrap()),
            pixel_format.stencil_bits.try_into().unwrap(),
            fb_info,
        );
        let surface = skia_safe::Surface::from_backend_render_target(
            gr_context,
            &backend_render_target,
            skia_safe::gpu::SurfaceOrigin::BottomLeft,
            skia_safe::ColorType::RGBA8888,
            None,
            None,
        )
        .unwrap();
        surface
    }
}

impl Drop for OpenGLSurface {
    fn drop(&mut self) {
        // Make sure that the context is current before Skia calls glDelete***
        self.opengl_context.make_current();
    }
}
