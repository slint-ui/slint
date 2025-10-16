// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{rc::Rc, sync::Arc};

use i_slint_core::{api::PlatformError, graphics::RequestedGraphicsAPI};
use i_slint_renderer_vello::VelloRenderer;

use crate::renderer::WinitCompatibleRenderer;

pub struct WinitVelloRenderer {
    renderer: VelloRenderer,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
}

impl WinitVelloRenderer {
    pub fn factory_for_graphics_api(
        requested_graphics_api: Option<&RequestedGraphicsAPI>,
    ) -> Result<
        fn(
            &Rc<crate::SharedBackendData>,
        ) -> Result<Box<dyn crate::WinitCompatibleRenderer>, PlatformError>,
        PlatformError,
    > {
        match requested_graphics_api {
            #[cfg(feature = "unstable-wgpu-26")]
            Some(RequestedGraphicsAPI::WGPU26()) => Ok(Self::new_suspended),
            None => Ok(Self::new_suspended),
            _ => Err(format!("The requested graphics API is not supported by Vello").into()),
        }
    }

    pub fn new_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn super::WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            renderer: VelloRenderer::new(),
            requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
        }))
    }
}

impl WinitCompatibleRenderer for WinitVelloRenderer {
    fn render(
        &self,
        _window: &i_slint_core::api::Window,
    ) -> Result<(), i_slint_core::api::PlatformError> {
        self.renderer.render()
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn suspend(&self) -> Result<(), i_slint_core::api::PlatformError> {
        self.renderer.suspend()
    }

    fn resume(
        &self,
        active_event_loop: &winit::event_loop::ActiveEventLoop,
        window_attributes: winit::window::WindowAttributes,
    ) -> Result<std::sync::Arc<winit::window::Window>, i_slint_core::api::PlatformError> {
        let winit_window = Arc::new(active_event_loop.create_window(window_attributes).map_err(
            |winit_os_error| {
                PlatformError::from(format!(
                    "Error creating native window for FemtoVG rendering: {}",
                    winit_os_error
                ))
            },
        )?);

        let size = winit_window.inner_size();

        self.renderer.resume(
            Box::new(winit_window.clone()),
            crate::winitwindowadapter::physical_size_to_slint(&size),
            self.requested_graphics_api.clone(),
        )?;

        Ok(winit_window)
    }
}
