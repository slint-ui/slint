// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Delegate the rendering to vello, through
//! [`i_slint_renderer_anyrender::VelloRenderer`].

use std::rc::Rc;
use std::sync::Arc;

use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::{DrawOutcome, Renderer};
use i_slint_renderer_anyrender::VelloRenderer;
use winit::event_loop::ActiveEventLoop;

use super::WinitCompatibleRenderer;

pub struct WinitVelloRenderer {
    renderer: VelloRenderer,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
}

impl WinitVelloRenderer {
    pub fn new_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn WinitCompatibleRenderer>, PlatformError> {
        if !i_slint_core::graphics::wgpu_29::any_wgpu29_adapters_with_gpu(
            shared_backend_data.requested_graphics_api.clone(),
        ) {
            return Err(PlatformError::from("WGPU: No GPU adapters found"));
        }
        Ok(Box::new(Self {
            renderer: VelloRenderer::new_vello(),
            requested_graphics_api: shared_backend_data.requested_graphics_api.clone(),
        }))
    }
}

impl WinitCompatibleRenderer for WinitVelloRenderer {
    fn render(&self, _window: &i_slint_core::api::Window) -> Result<DrawOutcome, PlatformError> {
        self.renderer.render()
    }

    fn as_core_renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn suspend(&self) -> Result<(), PlatformError> {
        // Also releases the winit window the callback holds on to.
        self.renderer.set_pre_present_callback(None);
        self.renderer.suspend_window();
        Ok(())
    }

    fn resume(
        &self,
        active_event_loop: &ActiveEventLoop,
        window_attributes: winit::window::WindowAttributes,
        _window_adapter_weak: std::rc::Weak<crate::winitwindowadapter::WinitWindowAdapter>,
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        let transparent = window_attributes.transparent;

        let winit_window = Arc::new(active_event_loop.create_window(window_attributes).map_err(
            |winit_os_error| {
                PlatformError::from(format!(
                    "Error creating native window for vello rendering: {winit_os_error}"
                ))
            },
        )?);

        let size = winit_window.inner_size();
        self.renderer.resume_window(
            winit_window.clone(),
            size.width.max(1),
            size.height.max(1),
            transparent,
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
