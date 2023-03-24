// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#[cfg(not(target_arch = "wasm32"))]
use glutin::{
    context::{ContextApi, ContextAttributesBuilder},
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use i_slint_core::{api::PhysicalSize, platform::PlatformError};
#[cfg(not(target_arch = "wasm32"))]
use raw_window_handle::HasRawWindowHandle;

pub struct OpenGLContext {
    #[cfg(not(target_arch = "wasm32"))]
    context: glutin::context::PossiblyCurrentContext,
    #[cfg(not(target_arch = "wasm32"))]
    surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
    #[cfg(target_arch = "wasm32")]
    canvas: web_sys::HtmlCanvasElement,
}

impl OpenGLContext {
    #[cfg(target_arch = "wasm32")]
    pub fn html_canvas_element(&self) -> web_sys::HtmlCanvasElement {
        self.canvas.clone()
    }

    pub fn ensure_current(&self) -> Result<(), PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        if !self.context.is_current() {
            self.context.make_current(&self.surface).map_err(|glutin_error| -> PlatformError {
                format!("FemtoVG: Error making context current: {glutin_error}").into()
            })?;
        }
        Ok(())
    }

    pub fn swap_buffers(&self) -> Result<(), PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        self.surface.swap_buffers(&self.context).map_err(|glutin_error| -> PlatformError {
            format!("FemtoVG: Error swapping buffers: {glutin_error}").into()
        })?;

        Ok(())
    }

    pub fn ensure_resized(&self, _size: PhysicalSize) -> Result<(), PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let width = _size.width.try_into().map_err(|_| {
                format!(
                    "Attempting to create window surface with an invalid width: {}",
                    _size.width
                )
            })?;
            let height = _size.height.try_into().map_err(|_| {
                format!(
                    "Attempting to create window surface with an invalid height: {}",
                    _size.height
                )
            })?;

            self.ensure_current()?;
            self.surface.resize(&self.context, width, height);
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_context<T>(
        window_builder: winit::window::WindowBuilder,
        window_target: &winit::event_loop::EventLoopWindowTarget<T>,
    ) -> Result<(winit::window::Window, Self), PlatformError> {
        let config_template_builder = glutin::config::ConfigTemplateBuilder::new();

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

        let width: std::num::NonZeroU32 = size.width.try_into().map_err(|e| {
            format!("Attempting to create window surface with width that doesn't fit into U32: {e}")
        })?;
        let height: std::num::NonZeroU32 = size.height.try_into().map_err(|e| {
            format!(
                "Attempting to create window surface with height that doesn't fit into U32: {e}"
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

        Ok((window, Self { context, surface }))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new_context<T>(
        window_builder: winit::window::WindowBuilder,
        window_target: &winit::event_loop::EventLoopWindowTarget<T>,
        canvas_id: &str,
    ) -> Result<(winit::window::Window, Self), PlatformError> {
        let window = window_builder.build(window_target).map_err(|winit_os_err| {
            format!(
                "FemtoVG Renderer: Could not create winit window wrapper for DOM canvas: {}",
                winit_os_err
            )
        })?;

        use wasm_bindgen::JsCast;

        let canvas = web_sys::window()
            .ok_or_else(|| "FemtoVG Renderer: Could not retrieve DOM window".to_string())?
            .document()
            .ok_or_else(|| "FemtoVG Renderer: Could not retrieve DOM document".to_string())?
            .get_element_by_id(canvas_id)
            .ok_or_else(|| {
                format!("FemtoVG Renderer: Could not retrieve existing HTML Canvas element '{canvas_id}'")
            })?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .map_err(|_| {
                format!("FemtoVG Renderer: Specified DOM element '{canvas_id}' is not a HTML Canvas")
            })?;

        Ok((window, Self { canvas }))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_proc_address(&self, name: &std::ffi::CStr) -> *const std::ffi::c_void {
        self.context.display().get_proc_address(name)
    }
}
