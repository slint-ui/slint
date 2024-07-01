// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::Cell;
use std::rc::Rc;

use crate::winitwindowadapter::physical_size_to_slint;
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

    pub fn new_opengl_suspended() -> Box<dyn super::WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::default_opengl(),
            suspended: Default::default(),
        })
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
            winit_window.scale_factor() as f32,
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
