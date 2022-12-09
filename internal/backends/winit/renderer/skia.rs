// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::rc::{Rc, Weak};

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::window::WindowAdapter;

pub struct SkiaRenderer {
    renderer: i_slint_renderer_skia::SkiaRenderer,
}

impl super::WinitCompatibleRenderer for SkiaRenderer {
    const NAME: &'static str = "Skia";

    fn new(window_adapter_weak: &Weak<dyn WindowAdapter>) -> Self {
        Self { renderer: i_slint_renderer_skia::SkiaRenderer::new(window_adapter_weak) }
    }

    fn show(&self, window: &Rc<winit::window::Window>) {
        let size: winit::dpi::PhysicalSize<u32> = window.inner_size();
        self.renderer.show(&window, &window, PhysicalWindowSize::new(size.width, size.height));
    }

    fn hide(&self) {
        self.renderer.hide();
    }

    fn render(&self, size: PhysicalWindowSize) {
        self.renderer.render(size);
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn resize_event(&self, size: PhysicalWindowSize) {
        self.renderer.resize_event(size)
    }
}
