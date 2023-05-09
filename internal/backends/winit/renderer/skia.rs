// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::rc::{Rc, Weak};

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;
use i_slint_core::window::WindowAdapter;

pub struct SkiaRenderer {
    renderer: i_slint_renderer_skia::SkiaRenderer<Rc<winit::window::Window>>,
}

impl super::WinitCompatibleRenderer for SkiaRenderer {
    const NAME: &'static str = "Skia";

    fn new(
        window_adapter_weak: &Weak<dyn WindowAdapter>,
        window_builder: winit::window::WindowBuilder,
    ) -> Result<(Self, Rc<winit::window::Window>), PlatformError> {
        let winit_window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_error| {
                format!("Error creating native window for Skia rendering: {}", winit_os_error)
            })
        })?);

        let size: winit::dpi::PhysicalSize<u32> = winit_window.inner_size();

        let width: u32 = size.width.try_into().map_err(|_| {
            format!(
                "Attempting to create a Skia window surface with an invalid width: {}",
                size.width
            )
        })?;
        let height: u32 = size.height.try_into().map_err(|_| {
            format!(
                "Attempting to create a Skia window surface with an invalid height: {}",
                size.height
            )
        })?;

        let renderer = i_slint_renderer_skia::SkiaRenderer::new(
            window_adapter_weak.clone(),
            winit_window.clone(),
            PhysicalWindowSize::new(width, height),
        )?;

        Ok((Self { renderer }, winit_window))
    }

    fn show(&self) -> Result<(), PlatformError> {
        self.renderer.show()
    }

    fn hide(&self) -> Result<(), PlatformError> {
        self.renderer.hide()
    }

    fn render(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        self.renderer.render(size)
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn resize_event(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        self.renderer.resize_event(size)
    }
}
