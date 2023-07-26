// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::rc::Rc;

use i_slint_core::platform::PlatformError;

use crate::WinitWindow;

pub struct SkiaRenderer {
    renderer: i_slint_renderer_skia::SkiaRenderer,
}

impl super::WinitCompatibleRenderer for SkiaRenderer {
    fn new(
        window_builder: winit::window::WindowBuilder,
    ) -> Result<(Self, Rc<WinitWindow>), PlatformError> {
        let winit_window = crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_error| {
                format!("Error creating native window for Skia rendering: {}", winit_os_error)
            })
        })?;

        let winit_window: Rc<WinitWindow> = Rc::new(winit_window.into());

        let renderer =
            i_slint_renderer_skia::SkiaRenderer::new(winit_window.clone(), winit_window.clone())?;

        Ok((Self { renderer }, winit_window))
    }

    fn render(&self, _window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        self.renderer.render()
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }
}
