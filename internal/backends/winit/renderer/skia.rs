// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;
use std::sync::Arc;

use crate::winitwindowadapter::physical_size_to_slint;
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::platform::PlatformError;
use i_slint_renderer_skia::SkiaRenderer;

pub struct WinitSkiaRenderer {
    renderer: SkiaRenderer,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
}

impl WinitSkiaRenderer {
    pub fn new_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn super::WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            renderer: SkiaRenderer::default(&shared_backend_data.skia_context),
            requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
        }))
    }

    #[cfg(not(target_os = "android"))]
    pub fn new_software_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn super::WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            renderer: SkiaRenderer::default_software(&shared_backend_data.skia_context),
            requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
        }))
    }

    #[cfg(not(ios_and_friends))]
    pub fn new_opengl_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn super::WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            renderer: SkiaRenderer::default_opengl(&shared_backend_data.skia_context),
            requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
        }))
    }

    #[cfg(target_vendor = "apple")]
    pub fn new_metal_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn super::WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            renderer: SkiaRenderer::default_metal(&shared_backend_data.skia_context),
            requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
        }))
    }

    #[cfg(feature = "renderer-skia-vulkan")]
    pub fn new_vulkan_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn super::WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            renderer: SkiaRenderer::default_vulkan(&shared_backend_data.skia_context),
            requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
        }))
    }

    #[cfg(target_family = "windows")]
    pub fn new_direct3d_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn super::WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            renderer: SkiaRenderer::default_direct3d(&shared_backend_data.skia_context),
            requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
        }))
    }

    #[cfg(feature = "unstable-wgpu-26")]
    pub fn new_wgpu_26_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn super::WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            renderer: SkiaRenderer::default_wgpu_26(&shared_backend_data.skia_context),
            requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
        }))
    }

    #[cfg(feature = "unstable-wgpu-27")]
    pub fn new_wgpu_27_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn super::WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            renderer: SkiaRenderer::default_wgpu_27(&shared_backend_data.skia_context),
            requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
        }))
    }

    pub fn factory_for_graphics_api(
        requested_graphics_api: Option<&RequestedGraphicsAPI>,
    ) -> Result<
        fn(
            &Rc<crate::SharedBackendData>,
        ) -> Result<Box<dyn crate::WinitCompatibleRenderer>, PlatformError>,
        PlatformError,
    > {
        match requested_graphics_api {
            Some(api) => {
                match api {
                    RequestedGraphicsAPI::OpenGL(_) => {
                        #[cfg(not(ios_and_friends))]
                        return Ok(Self::new_opengl_suspended);
                        #[cfg(ios_and_friends)]
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
                    #[cfg(feature = "unstable-wgpu-26")]
                    RequestedGraphicsAPI::WGPU26(..) => {
                        return Ok(Self::new_wgpu_26_suspended);
                    }
                    #[cfg(feature = "unstable-wgpu-27")]
                    RequestedGraphicsAPI::WGPU27(..) => {
                        return Ok(Self::new_wgpu_27_suspended);
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
        self.renderer.set_pre_present_callback(None);
        self.renderer.suspend()
    }

    fn resume(
        &self,
        active_event_loop: &winit::event_loop::ActiveEventLoop,
        window_attributes: winit::window::WindowAttributes,
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        let winit_window = Arc::new(active_event_loop.create_window(window_attributes).map_err(
            |winit_os_error| {
                PlatformError::from(format!(
                    "Error creating native window for Skia rendering: {}",
                    winit_os_error
                ))
            },
        )?);

        let size = winit_window.inner_size();

        self.renderer.set_window_handle(
            winit_window.clone(),
            winit_window.clone(),
            physical_size_to_slint(&size),
            self.requested_graphics_api.clone(),
        )?;

        self.renderer.set_pre_present_callback(Some(Box::new({
            let winit_window = winit_window.clone();
            move || {
                winit_window.pre_present_notify();
            }
        })));

        Ok(winit_window)
    }
}
