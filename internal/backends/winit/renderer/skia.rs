// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use std::rc::Weak;

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;
use i_slint_core::window::WindowAdapter;

pub struct SkiaRenderer {
    renderer: i_slint_renderer_skia::SkiaRenderer,
}

impl super::WinitCompatibleRenderer for SkiaRenderer {
    fn new(
        window_adapter_weak: &Weak<dyn WindowAdapter>,
        window_builder: winit::window::WindowBuilder,
    ) -> Result<(Self, winit::window::Window), PlatformError> {
        let winit_window = crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_error| {
                format!("Error creating native window for Skia rendering: {}", winit_os_error)
            })
        })?;

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
            &winit_window,
            &winit_window,
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
