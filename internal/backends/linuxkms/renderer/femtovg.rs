// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::num::NonZeroU32;

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
use i_slint_renderer_femtovg::FemtoVGRendererExt;
use raw_window_handle::{
    HasDisplayHandle, HasRawDisplayHandle, HasRawWindowHandle, HasWindowHandle,
};

use glutin::{
    context::{ContextApi, ContextAttributesBuilder},
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};

use crate::display::{egldisplay::EglDisplay, Presenter};

pub struct FemtoVGRendererAdapter {
    renderer: i_slint_renderer_femtovg::FemtoVGRenderer,
    size: PhysicalWindowSize,
}

struct GlContextWrapper {
    glutin_context: glutin::context::PossiblyCurrentContext,
    glutin_surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
    egl_display: EglDisplay,
}

impl GlContextWrapper {
    fn new(egl_display: EglDisplay) -> Result<Self, PlatformError> {
        let width: std::num::NonZeroU32 = egl_display.size.width.try_into().map_err(|_| {
            format!(
                "Attempting to create window surface with an invalid width: {}",
                egl_display.size.width
            )
        })?;
        let height: std::num::NonZeroU32 = egl_display.size.height.try_into().map_err(|_| {
            format!(
                "Attempting to create window surface with an invalid height: {}",
                egl_display.size.height
            )
        })?;

        let display_handle = egl_display.display_handle().unwrap();
        let window_handle = egl_display.window_handle().unwrap();

        let gl_display = unsafe {
            glutin::display::Display::new(
                display_handle.raw_display_handle(),
                glutin::display::DisplayApiPreference::Egl,
            )
            .map_err(|e| format!("Error creating EGL display: {e}"))?
        };

        let config_template = glutin::config::ConfigTemplateBuilder::new().build();

        let config = unsafe {
            gl_display
                .find_configs(config_template)
                .map_err(|e| format!("Error locating EGL configs: {e}"))?
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

        let gles_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(Some(glutin::context::Version {
                major: 2,
                minor: 0,
            })))
            .build(Some(window_handle.raw_window_handle()));

        let fallback_context_attributes =
            ContextAttributesBuilder::new().build(Some(window_handle.raw_window_handle()));

        let not_current_gl_context = unsafe {
            gl_display
                .create_context(&config, &gles_context_attributes)
                .or_else(|_| gl_display.create_context(&config, &fallback_context_attributes))
                .map_err(|e| format!("Error creating EGL context: {e}"))?
        };

        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            window_handle.raw_window_handle(),
            width,
            height,
        );

        let surface = unsafe {
            config
                .display()
                .create_window_surface(&config, &attrs)
                .map_err(|e| format!("Error creating EGL window surface: {e}"))?
        };

        let context = not_current_gl_context.make_current(&surface)
        .map_err(|glutin_error: glutin::error::Error| -> PlatformError {
            format!("FemtoVG Renderer: Failed to make newly created OpenGL context current: {glutin_error}")
            .into()
    })?;

        drop(window_handle);
        drop(display_handle);

        Ok(Self { glutin_context: context, glutin_surface: surface, egl_display })
    }
}

unsafe impl i_slint_renderer_femtovg::OpenGLInterface for GlContextWrapper {
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.glutin_context.is_current() {
            self.glutin_context.make_current(&self.glutin_surface).map_err(
                |glutin_error| -> PlatformError {
                    format!("FemtoVG: Error making context current: {glutin_error}").into()
                },
            )?;
        }
        Ok(())
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.glutin_surface.swap_buffers(&self.glutin_context).map_err(
            |glutin_error| -> PlatformError {
                format!("FemtoVG: Error swapping buffers: {glutin_error}").into()
            },
        )?;

        self.egl_display.present()
    }

    fn resize(
        &self,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.egl_display.size.height != height.get()
            || self.egl_display.size.width != width.get()
        {
            Err("Resizing a fullscreen window is not supported".into())
        } else {
            Ok(())
        }
    }

    fn get_proc_address(&self, name: &std::ffi::CStr) -> *const std::ffi::c_void {
        self.glutin_context.display().get_proc_address(name)
    }
}

impl FemtoVGRendererAdapter {
    pub fn new(
        device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let display = crate::display::egldisplay::create_egl_display(device_opener)?;

        let size = display.size;

        let renderer = Box::new(Self {
            renderer: i_slint_renderer_femtovg::FemtoVGRenderer::new(GlContextWrapper::new(
                display,
            )?)?,
            size,
        });

        eprintln!("Using FemtoVG OpenGL renderer");

        Ok(renderer)
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for FemtoVGRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }
    fn render_and_present(
        &self,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<(), PlatformError> {
        self.renderer.render_with_post_callback(Some(&|item_renderer| {
            draw_mouse_cursor_callback(item_renderer);
        }))
    }
    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }
}
