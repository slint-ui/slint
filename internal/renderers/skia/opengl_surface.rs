// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use std::{cell::RefCell, num::NonZeroU32};

use glutin::{
    config::GetGlConfig,
    context::{ContextApi, ContextAttributesBuilder},
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::{api::GraphicsAPI, platform::PlatformError};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

/// This surface type renders into the given window with OpenGL, using glutin and glow libraries.
pub struct OpenGLSurface {
    fb_info: skia_safe::gpu::gl::FramebufferInfo,
    surface: RefCell<skia_safe::Surface>,
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
    glutin_context: glutin::context::PossiblyCurrentContext,
    glutin_surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
}

impl super::Surface for OpenGLSurface {
    fn new(
        window_handle: raw_window_handle::WindowHandle<'_>,
        display_handle: raw_window_handle::DisplayHandle<'_>,
        size: PhysicalWindowSize,
    ) -> Result<Self, PlatformError> {
        let width: std::num::NonZeroU32 = size.width.try_into().map_err(|_| {
            format!("Attempting to create window surface with an invalid width: {}", size.width)
        })?;
        let height: std::num::NonZeroU32 = size.height.try_into().map_err(|_| {
            format!("Attempting to create window surface with an invalid height: {}", size.height)
        })?;

        let (current_glutin_context, glutin_surface) =
            Self::init_glutin(window_handle, display_handle, width, height)?;

        glutin_surface.resize(&current_glutin_context, width, height);

        let fb_info = {
            use glow::HasContext;

            let gl = unsafe {
                glow::Context::from_loader_function_cstr(|name| {
                    current_glutin_context.display().get_proc_address(name) as *const _
                })
            };
            let fboid = unsafe { gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING) };

            skia_safe::gpu::gl::FramebufferInfo {
                fboid: fboid.try_into().map_err(|_| {
                    format!("Skia Renderer: Internal error, framebuffer binding returned signed id")
                })?,
                format: skia_safe::gpu::gl::Format::RGBA8.into(),
                ..Default::default()
            }
        };

        let gl_interface = skia_safe::gpu::gl::Interface::new_load_with_cstr(|name| {
            current_glutin_context.display().get_proc_address(name) as *const _
        })
        .ok_or_else(|| {
            format!("Skia Renderer: Internal Error: Could not create OpenGL Interface")
        })?;

        let mut gr_context =
            skia_safe::gpu::DirectContext::new_gl(gl_interface, None).ok_or_else(|| {
                format!("Skia Renderer: Internal Error: Could not create Skia Direct Context from GL interface")
            })?;

        let width: i32 = size.width.try_into().map_err(|e| {
                format!("Attempting to create window surface with width that doesn't fit into non-zero i32: {e}")
            })?;
        let height: i32 = size.height.try_into().map_err(|e| {
                format!(
                    "Attempting to create window surface with height that doesn't fit into non-zero i32: {e}"
                )
            })?;

        let surface = Self::create_internal_surface(
            fb_info,
            &current_glutin_context,
            &mut gr_context,
            width,
            height,
        )?
        .into();

        Ok(Self {
            fb_info,
            surface,
            gr_context: RefCell::new(gr_context),
            glutin_context: current_glutin_context,
            glutin_surface,
        })
    }

    fn name(&self) -> &'static str {
        "opengl"
    }

    fn supports_graphics_api() -> bool {
        true
    }

    fn supports_graphics_api_with_self(&self) -> bool {
        true
    }

    fn with_graphics_api(&self, callback: &mut dyn FnMut(GraphicsAPI<'_>)) {
        let api = GraphicsAPI::NativeOpenGL {
            get_proc_address: &|name| {
                self.glutin_context.display().get_proc_address(name) as *const _
            },
        };
        callback(api)
    }

    fn with_active_surface(&self, callback: &mut dyn FnMut()) -> Result<(), PlatformError> {
        self.ensure_context_current()?;
        callback();
        Ok(())
    }

    fn render(
        &self,
        size: PhysicalWindowSize,
        callback: &dyn Fn(&skia_safe::Canvas, Option<&mut skia_safe::gpu::DirectContext>),
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<(), PlatformError> {
        self.ensure_context_current()?;

        let current_context = &self.glutin_context;

        let gr_context = &mut self.gr_context.borrow_mut();

        let mut surface = self.surface.borrow_mut();

        let width = size.width.try_into().ok();
        let height = size.height.try_into().ok();

        if let Some((width, height)) = width.zip(height) {
            if width != surface.width() || height != surface.height() {
                *surface = Self::create_internal_surface(
                    self.fb_info,
                    &current_context,
                    gr_context,
                    width,
                    height,
                )?;
            }
        }

        let skia_canvas = surface.canvas();

        skia_canvas.save();
        callback(skia_canvas, Some(gr_context));
        skia_canvas.restore();

        if let Some(pre_present_callback) = pre_present_callback.borrow_mut().as_mut() {
            pre_present_callback();
        }

        self.glutin_surface.swap_buffers(&current_context).map_err(|glutin_error| {
            format!("Skia OpenGL Renderer: Error swapping buffers: {glutin_error}").into()
        })
    }

    fn resize_event(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        self.ensure_context_current()?;

        if let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok()) {
            self.glutin_surface.resize(&self.glutin_context, width, height);
        }

        Ok(())
    }

    fn bits_per_pixel(&self) -> Result<u8, PlatformError> {
        let config = self.glutin_context.config();
        let rgb_bits = match config.color_buffer_type() {
            Some(glutin::config::ColorBufferType::Rgb { r_size, g_size, b_size }) => {
                r_size + g_size + b_size
            }
            other @ _ => {
                return Err(format!(
                    "Skia OpenGL Renderer: unsupported color buffer {other:?} encountered"
                )
                .into())
            }
        };
        Ok(rgb_bits + config.alpha_size())
    }
}

impl OpenGLSurface {
    fn init_glutin(
        _window_handle: raw_window_handle::WindowHandle<'_>,
        _display_handle: raw_window_handle::DisplayHandle<'_>,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> Result<
        (
            glutin::context::PossiblyCurrentContext,
            glutin::surface::Surface<glutin::surface::WindowSurface>,
        ),
        PlatformError,
    > {
        cfg_if::cfg_if! {
            if #[cfg(target_os = "macos")] {
                let display_api_preference = glutin::display::DisplayApiPreference::Cgl;
            } else if #[cfg(not(target_family = "windows"))] {
                let display_api_preference = glutin::display::DisplayApiPreference::Egl;
            } else {
                let display_api_preference = glutin::display::DisplayApiPreference::EglThenWgl(Some(_window_handle.raw_window_handle()));
            }
        }

        let gl_display = unsafe {
            glutin::display::Display::new(
                _display_handle.raw_display_handle(),
                display_api_preference,
            )
            .map_err(|glutin_error| {
                format!(
                    "Error creating glutin display for native display {:#?}: {}",
                    _display_handle.raw_display_handle(),
                    glutin_error
                )
            })?
        };

        let config_template_builder = glutin::config::ConfigTemplateBuilder::new();

        // On macOS, there's only one GL config and that's initialized based on the values in the config template
        // builder. So if that one has transparency enabled, it'll show up in the config, and will be set on the
        // context later. So we must enable it here, there's no way of enabling it later.
        // On EGL/GLX/WGL there are system provided configs that may or may not support transparency. Here in case
        // the system doesn't support transparency, we want to fall back to a config that doesn't - better than not
        // rendering anything at all. So we don't want to limit the configurations we get to see early on.
        // Commented out due to https://github.com/rust-windowing/glutin/issues/1640
        #[cfg(target_os = "macos")]
        let config_template_builder = config_template_builder.with_transparency(true);

        // Upstream advises to use this only on Windows.
        #[cfg(target_family = "windows")]
        let config_template_builder = config_template_builder
            .compatible_with_native_window(_window_handle.raw_window_handle());

        let config_template = config_template_builder.build();

        let config = unsafe {
            gl_display
                .find_configs(config_template)
                .map_err(|e| format!("Could not find valid OpenGL display configurations: {e}"))?
                .reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() < accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .ok_or("Unable to find suitable GL config")?
        };

        let create_gl_context = |gles_major| {
            let gles_context_attributes = ContextAttributesBuilder::new()
                .with_context_api(ContextApi::Gles(Some(glutin::context::Version {
                    major: gles_major,
                    minor: 0,
                })))
                .build(Some(_window_handle.raw_window_handle()));

            let fallback_context_attributes =
                ContextAttributesBuilder::new().build(Some(_window_handle.raw_window_handle()));

            unsafe {
                gl_display
                    .create_context(&config, &gles_context_attributes)
                    .or_else(|_| gl_display.create_context(&config, &fallback_context_attributes))
                    .map_err(|e| format!("Error creating OpenGL context: {e}"))
            }
        };

        let not_current_gl_context = create_gl_context(3).or_else(|_| create_gl_context(2))?;

        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            _window_handle.raw_window_handle(),
            width,
            height,
        );

        let surface = unsafe {
            config
                .display()
                .create_window_surface(&config, &attrs)
                .map_err(|e| format!("Error creating OpenGL window surface: {e}"))?
        };

        // Align the GL layer to the top-left, so that resizing only invalidates the bottom/right
        // part of the window.
        #[cfg(target_os = "macos")]
        if let raw_window_handle::RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle {
            ns_view,
            ..
        }) = _window_handle.raw_window_handle()
        {
            use cocoa::appkit::NSView;
            let view_id: cocoa::base::id = ns_view as *const _ as *mut _;
            unsafe {
                NSView::setLayerContentsPlacement(view_id, cocoa::appkit::NSViewLayerContentsPlacement::NSViewLayerContentsPlacementTopLeft)
            }
        }

        let context = not_current_gl_context.make_current(&surface)
            .map_err(|glutin_error: glutin::error::Error| -> PlatformError {
                format!("FemtoVG Renderer: Failed to make newly created OpenGL context current: {glutin_error}")
                .into()
        })?;

        // Sanity check, as all this might succeed on Windows without working GL drivers, but this will fail:
        if context
            .display()
            .get_proc_address(&std::ffi::CString::new("glCreateShader").unwrap())
            .is_null()
        {
            return Err(format!(
                "Failed to initialize OpenGL driver: Could not locate glCreateShader symbol"
            )
            .into());
        }

        // Try to default to vsync and ignore if the driver doesn't support it.
        surface
            .set_swap_interval(
                &context,
                glutin::surface::SwapInterval::Wait(NonZeroU32::new(1).unwrap()),
            )
            .ok();

        Ok((context, surface))
    }

    fn create_internal_surface(
        fb_info: skia_safe::gpu::gl::FramebufferInfo,
        gl_context: &glutin::context::PossiblyCurrentContext,
        gr_context: &mut skia_safe::gpu::DirectContext,
        width: i32,
        height: i32,
    ) -> Result<skia_safe::Surface, PlatformError> {
        let config = gl_context.config();

        let backend_render_target = skia_safe::gpu::backend_render_targets::make_gl(
            (width, height),
            Some(config.num_samples() as _),
            config.stencil_size() as _,
            fb_info,
        );
        match skia_safe::gpu::surfaces::wrap_backend_render_target(
            gr_context,
            &backend_render_target,
            skia_safe::gpu::SurfaceOrigin::BottomLeft,
            skia_safe::ColorType::RGBA8888,
            None,
            None,
        ) {
            Some(surface) => Ok(surface),
            None => {
                Err("Skia OpenGL Renderer: Failed to allocate internal backend rendering target"
                    .into())
            }
        }
    }

    fn ensure_context_current(&self) -> Result<(), PlatformError> {
        if !self.glutin_context.is_current() {
            self.glutin_context.make_current(&self.glutin_surface).map_err(
                |glutin_error| -> PlatformError {
                    format!("Skia Renderer: Error making context current: {glutin_error}").into()
                },
            )?;
        }
        Ok(())
    }
}

impl Drop for OpenGLSurface {
    fn drop(&mut self) {
        // Make sure that the context is current before Skia calls glDelete***
        // In the event that this fails for some reason (lost GL context), convey that to Skia so that it doesn't try to call
        // glDelete***
        if self.ensure_context_current().is_err() {
            i_slint_core::debug_log!("Skia OpenGL Renderer warning: Failed to make context current for destruction - considering context abandoned.");
            self.gr_context.borrow_mut().abandon();
        }
    }
}
