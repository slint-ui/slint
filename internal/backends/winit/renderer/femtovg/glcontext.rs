// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{num::NonZeroU32, sync::Arc};

use glutin::{
    config::GlConfig,
    context::{ContextApi, ContextAttributesBuilder},
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use i_slint_core::{graphics::RequestedOpenGLVersion, platform::PlatformError};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

pub struct OpenGLContext {
    context: glutin::context::PossiblyCurrentContext,
    surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
    winit_window: Arc<winit::window::Window>,
}

unsafe impl i_slint_renderer_femtovg::opengl::OpenGLInterface for OpenGLContext {
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.context.is_current() {
            self.context.make_current(&self.surface).map_err(|glutin_error| -> PlatformError {
                format!("FemtoVG: Error making context current: {glutin_error}").into()
            })?;
        }
        Ok(())
    }
    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.winit_window.pre_present_notify();

        self.surface.swap_buffers(&self.context).map_err(|glutin_error| -> PlatformError {
            format!("FemtoVG: Error swapping buffers: {glutin_error}").into()
        })?;

        Ok(())
    }

    fn resize(
        &self,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.ensure_current()?;
        self.surface.resize(&self.context, width, height);

        Ok(())
    }

    fn get_proc_address(&self, name: &std::ffi::CStr) -> *const std::ffi::c_void {
        self.context.display().get_proc_address(name)
    }
}

impl OpenGLContext {
    pub(crate) fn new_context(
        window_attributes: winit::window::WindowAttributes,
        active_event_loop: &winit::event_loop::ActiveEventLoop,
        requested_opengl_version: Option<RequestedOpenGLVersion>,
    ) -> Result<(Arc<winit::window::Window>, Self), PlatformError> {
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

        let display_builder = glutin_winit::DisplayBuilder::new()
            .with_preference(glutin_winit::ApiPreference::FallbackEgl)
            .with_window_attributes(Some(window_attributes.clone()));
        let config_picker = |it: Box<dyn Iterator<Item = glutin::config::Config> + '_>| {
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
        };
        let (window, gl_config) = display_builder
            .build(active_event_loop, config_template_builder, config_picker)
            .map_err(|glutin_err| {
                format!(
                    "Error creating OpenGL display ({:#?}) with glutin: {}",
                    active_event_loop.display_handle(),
                    glutin_err
                )
            })?;

        let gl_display = gl_config.display();

        let raw_window_handle = window
            .as_ref()
            .map(|w| w.window_handle())
            .transpose()
            .map_err(|err| {
                format!(
                    "Failed to retrieve a window handle while creating an OpenGL display: {err:?}"
                )
            })?
            .map(|h| h.as_raw());

        let requested_opengl_version =
            requested_opengl_version.unwrap_or(RequestedOpenGLVersion::OpenGLES(Some((2, 0))));
        let preferred_context_attributes = match requested_opengl_version {
            RequestedOpenGLVersion::OpenGL(version) => {
                let version =
                    version.map(|(major, minor)| glutin::context::Version { major, minor });
                ContextAttributesBuilder::new()
                    .with_context_api(ContextApi::OpenGl(version))
                    .build(raw_window_handle)
            }
            RequestedOpenGLVersion::OpenGLES(version) => {
                let version =
                    version.map(|(major, minor)| glutin::context::Version { major, minor });

                ContextAttributesBuilder::new()
                    .with_context_api(ContextApi::Gles(version))
                    .build(raw_window_handle)
            }
        };

        let fallback_context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);

        let not_current_gl_context = unsafe {
            gl_display
                .create_context(&gl_config, &preferred_context_attributes)
                .or_else(|_| gl_display.create_context(&gl_config, &fallback_context_attributes))
                .map_err(|glutin_err| format!("Cannot create OpenGL context: {glutin_err}"))?
        };

        let window = match window {
            Some(window) => window,
            None => glutin_winit::finalize_window(active_event_loop, window_attributes, &gl_config)
                .map_err(|winit_os_error| {
                    format!("Error finalizing window for OpenGL rendering: {winit_os_error}")
                })?,
        };

        let raw_window_handle = window.window_handle().map_err(|err| {
            format!("Failed to retrieve a window handle for window we just created: {err:?}")
        })?;

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
            raw_window_handle.as_raw(),
            width,
            height,
        );

        let surface = unsafe {
            gl_display.create_window_surface(&gl_config, &attrs).map_err(|glutin_err| {
                format!("Error creating OpenGL Window surface: {glutin_err}")
            })?
        };

        let context = not_current_gl_context.make_current(&surface)
            .map_err(|glutin_error: glutin::error::Error| -> PlatformError {
                format!("FemtoVG Renderer: Failed to make newly created OpenGL context current: {glutin_error}")
            .into()
        })?;

        // Align the GL layer to the top-left, so that resizing only invalidates the bottom/right
        // part of the window.
        #[cfg(target_os = "macos")]
        if let raw_window_handle::RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle {
            ns_view,
            ..
        }) = window
            .window_handle()
            .map_err(|e| {
                format!(
                    "Error obtaining window handle to adjust nsview layer contents placement: {e}"
                )
            })?
            .as_raw()
        {
            let ns_view: &objc2_app_kit::NSView = unsafe { ns_view.cast().as_ref() };
            unsafe {
                ns_view.setLayerContentsPlacement(
                    objc2_app_kit::NSViewLayerContentsPlacement::TopLeft,
                );
            }
        }

        // Sanity check, as all this might succeed on Windows without working GL drivers, but this will fail:
        if context
            .display()
            .get_proc_address(&std::ffi::CString::new("glCreateShader").unwrap())
            .is_null()
        {
            return Err(
                "Failed to initialize OpenGL driver: Could not locate glCreateShader symbol"
                    .to_string()
                    .into(),
            );
        }

        // Try to default to vsync and ignore if the driver doesn't support it.
        surface
            .set_swap_interval(
                &context,
                glutin::surface::SwapInterval::Wait(NonZeroU32::new(1).unwrap()),
            )
            .ok();

        let window = Arc::new(window);

        Ok((window.clone(), Self { context, surface, winit_window: window }))
    }
}
