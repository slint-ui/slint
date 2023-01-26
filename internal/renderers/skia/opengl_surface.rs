// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::RefCell;

use glutin::{
    config::GetGlConfig,
    context::{ContextApi, ContextAttributesBuilder},
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use i_slint_core::api::GraphicsAPI;
use i_slint_core::api::PhysicalSize as PhysicalWindowSize;

pub struct OpenGLSurface {
    fb_info: skia_safe::gpu::gl::FramebufferInfo,
    surface: RefCell<skia_safe::Surface>,
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
    glutin_context: glutin::context::PossiblyCurrentContext,
    glutin_surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
}

impl super::Surface for OpenGLSurface {
    const SUPPORTS_GRAPHICS_API: bool = true;

    fn new(
        window: &dyn raw_window_handle::HasRawWindowHandle,
        display: &dyn raw_window_handle::HasRawDisplayHandle,
        size: PhysicalWindowSize,
    ) -> Self {
        let (current_glutin_context, glutin_surface) = Self::init_glutin(window, display, size);

        let fb_info = {
            use glow::HasContext;

            let gl = unsafe {
                glow::Context::from_loader_function_cstr(|name| {
                    current_glutin_context.display().get_proc_address(name) as *const _
                })
            };
            let fboid = unsafe { gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING) };

            skia_safe::gpu::gl::FramebufferInfo {
                fboid: fboid.try_into().unwrap(),
                format: skia_safe::gpu::gl::Format::RGBA8.into(),
            }
        };

        let gl_interface = skia_safe::gpu::gl::Interface::new_load_with(|name| {
            current_glutin_context
                .display()
                .get_proc_address(&std::ffi::CString::new(name).unwrap()) as *const _
        });

        let mut gr_context = skia_safe::gpu::DirectContext::new_gl(gl_interface, None).unwrap();

        let surface =
            Self::create_internal_surface(fb_info, &current_glutin_context, &mut gr_context, size)
                .into();

        Self {
            fb_info,
            surface,
            gr_context: RefCell::new(gr_context),
            glutin_context: current_glutin_context,
            glutin_surface,
        }
    }

    fn name(&self) -> &'static str {
        "opengl"
    }

    fn with_graphics_api(&self, callback: impl FnOnce(GraphicsAPI<'_>)) {
        let api = GraphicsAPI::NativeOpenGL {
            get_proc_address: &|name| {
                self.glutin_context.display().get_proc_address(name) as *const _
            },
        };
        callback(api)
    }

    fn with_active_surface(&self, callback: impl FnOnce()) {
        self.ensure_context_current();
        callback();
    }

    fn render(
        &self,
        size: PhysicalWindowSize,
        callback: impl FnOnce(&mut skia_safe::Canvas, &mut skia_safe::gpu::DirectContext),
    ) {
        let width = size.width;
        let height = size.height;

        self.ensure_context_current();

        let current_context = &self.glutin_context;

        let gr_context = &mut self.gr_context.borrow_mut();

        let mut surface = self.surface.borrow_mut();
        if width != surface.width() as u32 || height != surface.height() as u32 {
            *surface =
                Self::create_internal_surface(self.fb_info, &current_context, gr_context, size);
        }

        let skia_canvas = surface.canvas();

        callback(skia_canvas, gr_context);

        self.glutin_surface.swap_buffers(&current_context).unwrap();
    }

    fn resize_event(&self, size: PhysicalWindowSize) {
        self.ensure_context_current();

        self.glutin_surface.resize(
            &self.glutin_context,
            size.width.try_into().unwrap(),
            size.height.try_into().unwrap(),
        );
    }

    fn bits_per_pixel(&self) -> u8 {
        let config = self.glutin_context.config();
        let rgb_bits = match config.color_buffer_type() {
            Some(glutin::config::ColorBufferType::Rgb { r_size, g_size, b_size }) => {
                r_size + g_size + b_size
            }
            _ => panic!("unsupported color buffer used with Skia OpenGL renderer"),
        };
        rgb_bits + config.alpha_size()
    }
}

impl OpenGLSurface {
    fn init_glutin(
        window: &dyn raw_window_handle::HasRawWindowHandle,
        display: &dyn raw_window_handle::HasRawDisplayHandle,
        size: PhysicalWindowSize,
    ) -> (
        glutin::context::PossiblyCurrentContext,
        glutin::surface::Surface<glutin::surface::WindowSurface>,
    ) {
        cfg_if::cfg_if! {
            if #[cfg(target_os = "macos")] {
                let pref = glutin::display::DisplayApiPreference::Cgl;
            } else if #[cfg(not(target_family = "windows"))] {
                let pref = glutin::display::DisplayApiPreference::Egl;
            } else {
                let pref = glutin::display::DisplayApiPreference::EglThenWgl(Some(window.raw_window_handle()));
            }
        }

        let gl_display =
            unsafe { glutin::display::Display::new(display.raw_display_handle(), pref).unwrap() };

        let config_template = glutin::config::ConfigTemplateBuilder::new()
            .compatible_with_native_window(window.raw_window_handle())
            .build();

        let config = unsafe {
            gl_display
                .find_configs(config_template)
                .unwrap()
                .reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() < accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .unwrap()
        };

        let gles_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(Some(glutin::context::Version {
                major: 2,
                minor: 0,
            })))
            .build(Some(window.raw_window_handle()));

        let fallback_context_attributes =
            ContextAttributesBuilder::new().build(Some(window.raw_window_handle()));

        let not_current_gl_context = unsafe {
            gl_display
                .create_context(&config, &gles_context_attributes)
                .or_else(|_| gl_display.create_context(&config, &fallback_context_attributes))
                .expect("failed to create context")
        };

        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            window.raw_window_handle(),
            size.width.try_into().unwrap(),
            size.height.try_into().unwrap(),
        );

        let surface = unsafe { config.display().create_window_surface(&config, &attrs).unwrap() };

        #[cfg(target_os = "macos")]
        if let raw_window_handle::RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle {
            ns_view,
            ..
        }) = window.raw_window_handle()
        {
            use cocoa::appkit::NSView;
            let view_id: cocoa::base::id = ns_view as *const _ as *mut _;
            unsafe {
                NSView::setLayerContentsPlacement(view_id, cocoa::appkit::NSViewLayerContentsPlacement::NSViewLayerContentsPlacementTopLeft)
            }
        }

        (not_current_gl_context.make_current(&surface).unwrap(), surface)
    }

    fn create_internal_surface(
        fb_info: skia_safe::gpu::gl::FramebufferInfo,
        gl_context: &glutin::context::PossiblyCurrentContext,
        gr_context: &mut skia_safe::gpu::DirectContext,
        size: PhysicalWindowSize,
    ) -> skia_safe::Surface {
        let config = gl_context.config();
        let backend_render_target = skia_safe::gpu::BackendRenderTarget::new_gl(
            (size.width.try_into().unwrap(), size.height.try_into().unwrap()),
            Some(config.num_samples() as _),
            config.stencil_size() as _,
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

    fn ensure_context_current(&self) {
        if !self.glutin_context.is_current() {
            self.glutin_context.make_current(&self.glutin_surface).unwrap();
        }
    }
}

impl Drop for OpenGLSurface {
    fn drop(&mut self) {
        // Make sure that the context is current before Skia calls glDelete***
        self.ensure_context_current();
    }
}
