// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use glutin::{
    context::{ContextApi, ContextAttributesBuilder},
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use i_slint_core::{api::PhysicalSize, platform::PlatformError};
use raw_window_handle::HasRawWindowHandle;

pub struct OpenGLContext {
    context: glutin::context::PossiblyCurrentContext,
    surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
}

unsafe impl i_slint_renderer_femtovg::OpenGLContextWrapper for OpenGLContext {
    fn ensure_current(&self) -> Result<(), PlatformError> {
        if !self.context.is_current() {
            self.context.make_current(&self.surface).map_err(|glutin_error| -> PlatformError {
                format!("FemtoVG: Error making context current: {glutin_error}").into()
            })?;
        }
        Ok(())
    }
    fn swap_buffers(&self) -> Result<(), PlatformError> {
        self.surface.swap_buffers(&self.context).map_err(|glutin_error| -> PlatformError {
            format!("FemtoVG: Error swapping buffers: {glutin_error}").into()
        })?;

        Ok(())
    }

    fn resize(&self, _size: PhysicalSize) -> Result<(), PlatformError> {
        let width = _size.width.try_into().map_err(|_| {
            format!("Attempting to create window surface with an invalid width: {}", _size.width)
        })?;
        let height = _size.height.try_into().map_err(|_| {
            format!("Attempting to create window surface with an invalid height: {}", _size.height)
        })?;

        self.ensure_current()?;
        self.surface.resize(&self.context, width, height);

        Ok(())
    }

    fn get_proc_address(&self, name: &std::ffi::CStr) -> *const std::ffi::c_void {
        self.context.display().get_proc_address(name)
    }
}

impl OpenGLContext {
    pub fn new_context<T>(
        window_builder: winit::window::WindowBuilder,
        window_target: &winit::event_loop::EventLoopWindowTarget<T>,
    ) -> Result<(winit::window::Window, Self), PlatformError> {
        let config_template_builder = glutin::config::ConfigTemplateBuilder::new();

        // On macOS, there's only one GL config and that's initialized based on the values in the config template
        // builder. So if that one has transparency enabled, it'll show up in the config, and will be set on the
        // context later. So we must enable it here, there's no way of enabling it later.
        // On EGL/GLX/WGL there are system provided configs that may or may not support transparency. Here in case
        // the system doesn't support transparency, we want to fall back to a config that doesn't - better than not
        // rendering anything at all. So we don't want to limit the configurations we get to see early on.
        #[cfg(target_os = "macos")]
        let config_template_builder = config_template_builder.with_transparency(true);

        let (window, gl_config) = glutin_winit::DisplayBuilder::new()
            .with_preference(glutin_winit::ApiPrefence::FallbackEgl)
            .with_window_builder(Some(window_builder.clone()))
            .build(window_target, config_template_builder, |it| {
                it.reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() < accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .expect("internal error: Could not find any matching GL configuration")
            })
            .map_err(|glutin_err| {
                format!("Error creating OpenGL display with glutin: {}", glutin_err)
            })?;

        let gl_display = gl_config.display();

        let raw_window_handle = window.as_ref().map(|w| w.raw_window_handle());

        let gles_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(Some(glutin::context::Version {
                major: 2,
                minor: 0,
            })))
            .build(raw_window_handle);

        let fallback_context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);

        let not_current_gl_context = unsafe {
            gl_display
                .create_context(&gl_config, &gles_context_attributes)
                .or_else(|_| gl_display.create_context(&gl_config, &fallback_context_attributes))
                .map_err(|glutin_err| format!("Cannot create OpenGL context: {}", glutin_err))?
        };

        let window = match window {
            Some(window) => window,
            None => glutin_winit::finalize_window(window_target, window_builder, &gl_config)
                .map_err(|winit_os_error| {
                    format!("Error finalizing window for OpenGL rendering: {}", winit_os_error)
                })?,
        };

        let size: winit::dpi::PhysicalSize<u32> = window.inner_size();

        let width: std::num::NonZeroU32 = size.width.try_into().map_err(|_| {
            format!(
                "Attempting to create an OpenGL window surface with an invalid width: {}",
                size.width
            )
        })?;
        let height: std::num::NonZeroU32 = size.height.try_into().map_err(|_| {
            format!(
                "Attempting to create an OpenGL window surface with an invalid height: {}",
                size.height
            )
        })?;

        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            window.raw_window_handle(),
            width,
            height,
        );

        let surface = unsafe {
            gl_display.create_window_surface(&gl_config, &attrs).map_err(|glutin_err| {
                format!("Error creating OpenGL Window surface: {}", glutin_err)
            })?
        };

        // Align the GL layer to the top-left, so that resizing only invalidates the bottom/right
        // part of the window.
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

        Ok((window, Self { context, surface }))
    }
}
