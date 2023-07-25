// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::Renderer;
use i_slint_renderer_femtovg::FemtoVGRenderer;

#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

#[cfg(not(target_arch = "wasm32"))]
mod glcontext;

pub struct GlutinFemtoVGRenderer {
    renderer: FemtoVGRenderer,
}

impl super::WinitCompatibleRenderer for GlutinFemtoVGRenderer {
    fn new(
        window_builder: winit::window::WindowBuilder,
    ) -> Result<(Self, winit::window::Window), PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        let (winit_window, opengl_context) = crate::event_loop::with_window_target(|event_loop| {
            glcontext::OpenGLContext::new_context(window_builder, event_loop.event_loop_target())
        })?;

        #[cfg(target_arch = "wasm32")]
        let winit_window = crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_err| {
                format!(
                    "FemtoVG Renderer: Could not create winit window wrapper for DOM canvas: {}",
                    winit_os_err
                )
            })
        })?;

        let renderer = FemtoVGRenderer::new(
            #[cfg(not(target_arch = "wasm32"))]
            opengl_context,
            #[cfg(target_arch = "wasm32")]
            winit_window.canvas(),
        )?;

        Ok((Self { renderer }, winit_window))
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
}
