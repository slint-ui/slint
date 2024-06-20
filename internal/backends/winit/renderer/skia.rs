// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use crate::winitwindowadapter::physical_size_to_slint;
use i_slint_core::platform::PlatformError;

pub struct WinitSkiaRenderer {
    renderer: i_slint_renderer_skia::SkiaRenderer,
}

impl WinitSkiaRenderer {
    pub fn new(
        window_attributes: winit::window::WindowAttributes,
    ) -> Result<(Box<dyn super::WinitCompatibleRenderer>, Rc<winit::window::Window>), PlatformError>
    {
        let winit_window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
            event_loop.create_window(window_attributes).map_err(|winit_os_error| {
                format!("Error creating native window for Skia rendering: {}", winit_os_error)
                    .into()
            })
        })?);

        let renderer = i_slint_renderer_skia::SkiaRenderer::default();

        renderer.set_pre_present_callback(Some(Box::new({
            let winit_window = winit_window.clone();
            move || {
                winit_window.pre_present_notify();
            }
        })));

        Ok((Box::new(Self { renderer }), winit_window))
    }

    #[cfg(not(target_os = "android"))]
    pub fn new_software(
        window_attributes: winit::window::WindowAttributes,
    ) -> Result<(Box<dyn super::WinitCompatibleRenderer>, Rc<winit::window::Window>), PlatformError>
    {
        let winit_window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
            event_loop.create_window(window_attributes).map_err(|winit_os_error| {
                format!("Error creating native window for Skia rendering: {}", winit_os_error)
                    .into()
            })
        })?);

        let renderer = i_slint_renderer_skia::SkiaRenderer::default_software();

        renderer.set_pre_present_callback(Some(Box::new({
            let winit_window = winit_window.clone();
            move || {
                winit_window.pre_present_notify();
            }
        })));

        Ok((Box::new(Self { renderer }), winit_window))
    }

    pub fn new_opengl(
        window_attributes: winit::window::WindowAttributes,
    ) -> Result<(Box<dyn super::WinitCompatibleRenderer>, Rc<winit::window::Window>), PlatformError>
    {
        let winit_window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
            event_loop.create_window(window_attributes).map_err(|winit_os_error| {
                format!("Error creating native window for Skia rendering: {}", winit_os_error)
                    .into()
            })
        })?);

        let renderer = i_slint_renderer_skia::SkiaRenderer::default_opengl();

        renderer.set_pre_present_callback(Some(Box::new({
            let winit_window = winit_window.clone();
            move || {
                winit_window.pre_present_notify();
            }
        })));

        Ok((Box::new(Self { renderer }), winit_window))
    }
}

impl super::WinitCompatibleRenderer for WinitSkiaRenderer {
    fn render(&self, _window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        self.renderer.render()
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn resumed(&self, winit_window: Rc<winit::window::Window>) -> Result<(), PlatformError> {
        let size = winit_window.inner_size();

        self.renderer.set_window_handle(
            winit_window.clone(),
            winit_window.clone(),
            physical_size_to_slint(&size),
            winit_window.scale_factor() as f32,
        )
    }

    fn grab_window(
        &self,
        _window: &i_slint_core::api::Window,
    ) -> Result<i_slint_core::graphics::SharedImageBuffer, PlatformError> {
        self.renderer.screenshot()
    }
}
