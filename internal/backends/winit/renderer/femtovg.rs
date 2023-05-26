// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::Renderer;
use i_slint_renderer_femtovg::FemtoVGRenderer;

mod glcontext;

pub struct GlutinFemtoVGRenderer {
    renderer: FemtoVGRenderer,
}

impl super::WinitCompatibleRenderer for GlutinFemtoVGRenderer {
    fn new(
        window_builder: winit::window::WindowBuilder,
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> Result<(Self, winit::window::Window), PlatformError> {
        let (winit_window, opengl_context) = crate::event_loop::with_window_target(|event_loop| {
            glcontext::OpenGLContext::new_context(
                window_builder,
                event_loop.event_loop_target(),
                #[cfg(target_arch = "wasm32")]
                canvas_id,
            )
        })?;

        let renderer = FemtoVGRenderer::new(opengl_context)?;

        Ok((Self { renderer }, winit_window))
    }

    fn show(&self) -> Result<(), PlatformError> {
        self.renderer.show()
    }

    fn hide(&self) -> Result<(), PlatformError> {
        self.renderer.hide()
    }

    fn render(&self, window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        self.renderer.render(window)
    }

    fn as_core_renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn resize_event(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        self.renderer.resize(size)
    }

    #[cfg(target_arch = "wasm32")]
    fn html_canvas_element(&self) -> web_sys::HtmlCanvasElement {
        self.renderer.html_canvas_element()
    }
}
