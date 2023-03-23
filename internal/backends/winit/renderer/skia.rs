// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::rc::{Rc, Weak};

use glutin::platform::x11::X11GlConfigExt;
use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::window::WindowAdapter;
use winit::platform::x11::WindowBuilderExtX11;

pub struct SkiaRenderer {
    renderer: i_slint_renderer_skia::SkiaRenderer<Rc<winit::window::Window>>,
}

impl super::WinitCompatibleRenderer for SkiaRenderer {
    const NAME: &'static str = "Skia";

    fn new(window_adapter_weak: &Weak<dyn WindowAdapter>) -> Self {
        crate::event_loop::with_window_target(|event_loop| Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::new(
                Some(event_loop.event_loop_target()),
                window_adapter_weak.clone(),
            ),
        })
    }

    fn show(&self, window_builder: winit::window::WindowBuilder) -> Rc<winit::window::Window> {
        let window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
            #[cfg(feature = "x11")]
            let window_builder = if let Some(x11_visual) =
                self.renderer.glutin_config().and_then(|config| config.x11_visual())
            {
                window_builder.with_x11_visual(x11_visual.into_raw())
            } else {
                window_builder
            };

            window_builder.build(event_loop.event_loop_target()).unwrap()
        }));

        let size: winit::dpi::PhysicalSize<u32> = window.inner_size();
        self.renderer.show(window.clone(), PhysicalWindowSize::new(size.width, size.height));

        window
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
