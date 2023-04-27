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

    fn new(window_adapter_weak: &Weak<dyn WindowAdapter>) -> Self {
        Self { renderer: i_slint_renderer_skia::SkiaRenderer::new(window_adapter_weak.clone()) }
    }

    fn show(
        &self,
        window_builder: winit::window::WindowBuilder,
    ) -> Result<Rc<winit::window::Window>, PlatformError> {
        let window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).unwrap()
        }));

        let size: winit::dpi::PhysicalSize<u32> = window.inner_size();
        self.renderer.show(window.clone(), PhysicalWindowSize::new(size.width, size.height))?;

        Ok(window)
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
