// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use std::{num::NonZeroU32, rc::Rc};

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

use crate::display::{gbmdisplay::GbmDisplay, Presenter, RenderingRotation};
use crate::drmoutput::DrmOutput;

pub struct FemtoVGRendererAdapter {
    renderer: i_slint_renderer_femtovg::FemtoVGRenderer,
    gbm_display: Rc<GbmDisplay>,
}

struct GlContextWrapper {
    glutin_context: glutin::context::PossiblyCurrentContext,
    glutin_surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
}

impl GlContextWrapper {
    fn new(gbm_display: &GbmDisplay) -> Result<Self, PlatformError> {
        let (width, height) = gbm_display.drm_output.size();
        let width: std::num::NonZeroU32 = width.try_into().map_err(|_| {
            format!("Attempting to create window surface with an invalid width: {}", width)
        })?;
        let height: std::num::NonZeroU32 = height.try_into().map_err(|_| {
            format!("Attempting to create window surface with an invalid height: {}", height)
        })?;

        let display_handle = gbm_display.display_handle().unwrap();
        let window_handle = gbm_display.window_handle().unwrap();

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

        Ok(Self { glutin_context: context, glutin_surface: surface })
    }
}

unsafe impl i_slint_renderer_femtovg::OpenGLInterface for GlContextWrapper {
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.glutin_context.is_current() {
            self.glutin_context.make_current(&self.glutin_surface).map_err(
                |glutin_error| -> PlatformError {
                    format!("FemtoVG: Error making context current: {glutin_error}").into()
                },
            )?;
        }
        Ok(())
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.glutin_surface.swap_buffers(&self.glutin_context).map_err(
            |glutin_error| -> PlatformError {
                format!("FemtoVG: Error swapping buffers: {glutin_error}").into()
            },
        )?;
        Ok(())
    }

    fn resize(
        &self,
        _width: NonZeroU32,
        _height: NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Ignore resize requests
        Ok(())
    }

    fn get_proc_address(&self, name: &std::ffi::CStr) -> *const std::ffi::c_void {
        self.glutin_context.display().get_proc_address(name)
    }
}

impl FemtoVGRendererAdapter {
    pub fn new(
        device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let drm_output = DrmOutput::new(device_opener)?;
        let egl_display = Rc::new(crate::display::gbmdisplay::GbmDisplay::new(drm_output)?);

        let renderer = Box::new(Self {
            renderer: i_slint_renderer_femtovg::FemtoVGRenderer::new(GlContextWrapper::new(
                &egl_display,
            )?)?,
            gbm_display: egl_display,
        });

        eprintln!("Using FemtoVG OpenGL renderer");

        Ok(renderer)
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for FemtoVGRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn is_ready_to_present(&self) -> bool {
        self.gbm_display.is_ready_to_present()
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
        ready_for_next_animation_frame: Box<dyn FnOnce()>,
    ) -> Result<(), PlatformError> {
        let size = self.size();
        self.renderer.render_transformed_with_post_callback(
            rotation.degrees(),
            rotation.translation_after_rotation(size),
            size,
            Some(&|item_renderer| {
                draw_mouse_cursor_callback(item_renderer);
            }),
        )?;
        self.gbm_display.present_with_next_frame_callback(ready_for_next_animation_frame)?;
        Ok(())
    }
    fn size(&self) -> i_slint_core::api::PhysicalSize {
        let (width, height) = self.gbm_display.drm_output.size();
        i_slint_core::api::PhysicalSize::new(width, height)
    }

    fn register_page_flip_handler(
        &self,
        event_loop_handle: crate::calloop_backend::EventLoopHandle,
    ) -> Result<(), PlatformError> {
        self.gbm_display.clone().register_page_flip_handler(event_loop_handle)
    }
}
