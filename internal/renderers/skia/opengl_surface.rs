// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

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
            }
        };

        let gl_interface = skia_safe::gpu::gl::Interface::new_load_with_cstr(|name| {
            current_glutin_context.display().get_proc_address(name) as *const _
        });

        let mut gr_context =
            skia_safe::gpu::DirectContext::new_gl(gl_interface, None).ok_or_else(|| {
                format!("Skia Renderer: Internal Error: Could not create Skia OpenGL interface")
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

    fn with_graphics_api(&self, callback: impl FnOnce(GraphicsAPI<'_>)) {
        let api = GraphicsAPI::NativeOpenGL {
            get_proc_address: &|name| {
                self.glutin_context.display().get_proc_address(name) as *const _
            },
        };
        callback(api)
    }

    fn with_active_surface(&self, callback: impl FnOnce()) -> Result<(), PlatformError> {
        self.ensure_context_current()?;
        callback();
        Ok(())
    }

    fn render(
        &self,
        size: PhysicalWindowSize,
        callback: impl FnOnce(&mut skia_safe::Canvas, &mut skia_safe::gpu::DirectContext),
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

        callback(skia_canvas, gr_context);

        self.glutin_surface.swap_buffers(&current_context).map_err(|glutin_error| {
            format!("Skia OpenGL Renderer: Error swapping buffers: {glutin_error}").into()
        })
    }

    fn resize_event(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        self.ensure_context_current()?;

        let width = size.width.try_into().map_err(|_| {
            format!(
                "Attempting to resize OpenGL window surface with an invalid width: {}",
                size.width
            )
        })?;
        let height = size.height.try_into().map_err(|_| {
            format!(
                "Attempting to resize OpenGL window surface with an invalid height: {}",
                size.height
            )
        })?;

        self.glutin_surface.resize(&self.glutin_context, width, height);
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
                let prefs = [glutin::display::DisplayApiPreference::Cgl];
            } else if #[cfg(all(feature = "x11", not(target_family = "windows")))] {
                let prefs = [glutin::display::DisplayApiPreference::Egl, glutin::display::DisplayApiPreference::Glx(Box::new(winit::platform::x11::register_xlib_error_hook))];
            } else if #[cfg(not(target_family = "windows"))] {
                let prefs = [glutin::display::DisplayApiPreference::Egl];
            } else {
                let prefs = [glutin::display::DisplayApiPreference::EglThenWgl(Some(_window_handle))];
            }
        }

        let try_create_surface =
            |display_api_preference| -> Result<(_, _), Box<dyn std::error::Error>> {
                let gl_display = unsafe {
                    glutin::display::Display::new(
                        _display_handle.raw_display_handle(),
                        display_api_preference,
                    )?
                };

                let config_template_builder = glutin::config::ConfigTemplateBuilder::new();

                // On macOS, there's only one GL config and that's initialized based on the values in the config template
                // builder. So if that one has transparency enabled, it'll show up in the config, and will be set on the
                // context later. So we must enable it here, there's no way of enabling it later.
                // On EGL/GLX/WGL there are system provided configs that may or may not support transparency. Here in case
                // the system doesn't support transparency, we want to fall back to a config that doesn't - better than not
                // rendering anything at all. So we don't want to limit the configurations we get to see early on.
                #[cfg(target_os = "macos")]
                let config_template_builder = config_template_builder.with_transparency(true);

                // Upstream advises to use this only on Windows.
                #[cfg(target_family = "windows")]
                let config_template_builder =
                    config_template_builder.compatible_with_native_window(_window_handle);

                let config_template = config_template_builder.build();

                let config = unsafe {
                    gl_display
                        .find_configs(config_template)?
                        .reduce(|accum, config| {
                            let transparency_check =
                                config.supports_transparency().unwrap_or(false)
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
                    .build(Some(_window_handle.raw_window_handle()));

                let fallback_context_attributes =
                    ContextAttributesBuilder::new().build(Some(_window_handle.raw_window_handle()));

                let not_current_gl_context = unsafe {
                    gl_display.create_context(&config, &gles_context_attributes).or_else(|_| {
                        gl_display.create_context(&config, &fallback_context_attributes)
                    })?
                };

                let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
                    _window_handle.raw_window_handle(),
                    width,
                    height,
                );

                let surface = unsafe { config.display().create_window_surface(&config, &attrs)? };

                Ok((surface, not_current_gl_context))
            };

        let num_prefs = prefs.len();
        let (surface, not_current_gl_context) = prefs
            .into_iter()
            .enumerate()
            .find_map(|(i, pref)| {
                let is_last = i == num_prefs - 1;

                match try_create_surface(pref) {
                    Ok(result) => Some(Ok(result)),
                    Err(glutin_error) => {
                        if is_last {
                            return Some(Err(format!("Skia OpenGL Renderer: Failed to create OpenGL Window Surface: {glutin_error}")));
                        }
                        None
                    }
                }
            })
            .unwrap()?;

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

        let backend_render_target = skia_safe::gpu::BackendRenderTarget::new_gl(
            (width, height),
            Some(config.num_samples() as _),
            config.stencil_size() as _,
            fb_info,
        );
        match skia_safe::Surface::from_backend_render_target(
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
        self.ensure_context_current().expect("Skia OpenGL Renderer: Failed to make OpenGL context current before deleting graphics resources");
    }
}
