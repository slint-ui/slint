// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::Cell;
use std::rc::Rc;

use crate::winitwindowadapter::physical_size_to_slint;
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::platform::PlatformError;

pub struct WinitSkiaRenderer {
    renderer: i_slint_renderer_skia::SkiaRenderer,
    suspended: Cell<bool>,
}

impl WinitSkiaRenderer {
    pub fn new_suspended() -> Box<dyn super::WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::default(),
            suspended: Default::default(),
        })
    }

    #[cfg(not(target_os = "android"))]
    pub fn new_software_suspended() -> Box<dyn super::WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::default_software(),
            suspended: Default::default(),
        })
    }

    #[cfg(not(target_os = "ios"))]
    pub fn new_opengl_suspended() -> Box<dyn super::WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::default_opengl(),
            suspended: Default::default(),
        })
    }

    #[cfg(target_vendor = "apple")]
    pub fn new_metal_suspended() -> Box<dyn super::WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::default_metal(),
            suspended: Default::default(),
        })
    }

    #[cfg(feature = "renderer-skia-vulkan")]
    pub fn new_vulkan_suspended() -> Box<dyn super::WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::default_vulkan(),
            suspended: Default::default(),
        })
    }

    #[cfg(target_family = "windows")]
    pub fn new_direct3d_suspended() -> Box<dyn super::WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::default_direct3d(),
            suspended: Default::default(),
        })
    }

    pub fn factory_for_graphics_api(
        requested_graphics_api: Option<&RequestedGraphicsAPI>,
    ) -> Result<fn() -> Box<dyn crate::WinitCompatibleRenderer>, PlatformError> {
        match requested_graphics_api {
            Some(api) => {
                match api {
                    RequestedGraphicsAPI::OpenGL(_) => {
                        #[cfg(not(target_os = "ios"))]
                        return Ok(Self::new_opengl_suspended);
                        #[cfg(target_os = "ios")]
                        return Err(format!(
                            "OpenGL rendering requested but this is not supported on iOS"
                        )
                        .into());
                    }
                    RequestedGraphicsAPI::Metal => {
                        #[cfg(target_vendor = "apple")]
                        return Ok(Self::new_metal_suspended);
                        #[cfg(not(target_vendor = "apple"))]
                        return Err(format!("Metal rendering requested but this is only supported on Apple platforms").into());
                    }
                    RequestedGraphicsAPI::Vulkan => {
                        #[cfg(feature = "renderer-skia-vulkan")]
                        return Ok(Self::new_vulkan_suspended);
                        #[cfg(not(feature = "renderer-skia-vulkan"))]
                        return Err(format!(
                            "Vulkan rendering requested but renderer-skia-vulkan is not enabled"
                        )
                        .into());
                    }
                    RequestedGraphicsAPI::Direct3D => {
                        #[cfg(target_family = "windows")]
                        return Ok(Self::new_direct3d_suspended);
                        #[cfg(not(target_family = "windows"))]
                        return Err(format!(
                            "Direct3D rendering requested but this is only supported on Windows"
                        )
                        .into());
                    }
                }
            }
            None => Ok(Self::new_suspended),
        }
    }
}

impl super::WinitCompatibleRenderer for WinitSkiaRenderer {
    fn render(&self, _window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        self.renderer.render()
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn suspend(&self) -> Result<(), PlatformError> {
        self.suspended.set(true);
        self.renderer.set_pre_present_callback(None);
        self.renderer.suspend()
    }

    fn resume(
        &self,
        window_attributes: winit::window::WindowAttributes,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Rc<winit::window::Window>, PlatformError> {
        let winit_window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
            event_loop.create_window(window_attributes).map_err(|winit_os_error| {
                format!("Error creating native window for Skia rendering: {}", winit_os_error)
                    .into()
            })
        })?);

        let size = winit_window.inner_size();

        self.renderer.set_window_handle(
            winit_window.clone(),
            winit_window.clone(),
            physical_size_to_slint(&size),
            requested_graphics_api,
        )?;

        self.renderer.set_pre_present_callback(Some(Box::new({
            let winit_window = winit_window.clone();
            move || {
                winit_window.pre_present_notify();
            }
        })));

        self.suspended.set(false);

        Ok(winit_window)
    }

    fn is_suspended(&self) -> bool {
        self.suspended.get()
    }
}
