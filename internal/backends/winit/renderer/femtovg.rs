// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use std::rc::Rc;

use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::Renderer;
use i_slint_renderer_femtovg::FemtoVGRenderer;

#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

use super::WinitCompatibleRenderer;

#[cfg(not(target_arch = "wasm32"))]
mod glcontext;

pub struct GlutinFemtoVGRenderer {
    renderer: FemtoVGRenderer,
}

impl GlutinFemtoVGRenderer {
    pub fn new(
        window_builder: winit::window::WindowBuilder,
    ) -> Result<(Box<dyn WinitCompatibleRenderer>, Rc<winit::window::Window>), PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        let (winit_window, opengl_context) = crate::event_loop::with_window_target(|event_loop| {
            Ok(glcontext::OpenGLContext::new_context(
                window_builder,
                event_loop.event_loop_target(),
            )?)
        })?;

        #[cfg(target_arch = "wasm32")]
        let winit_window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_err| {
                format!(
                    "FemtoVG Renderer: Could not create winit window wrapper for DOM canvas: {}",
                    winit_os_err
                )
                .into()
            })
        })?);

        let renderer = FemtoVGRenderer::new(
            #[cfg(not(target_arch = "wasm32"))]
            opengl_context,
            #[cfg(target_arch = "wasm32")]
            winit_window
                .canvas()
                .ok_or_else(|| "FemtoVG Renderer: winit didn't return a canvas")?,
        )?;

        Ok((Box::new(Self { renderer }), winit_window))
    }
}

impl super::WinitCompatibleRenderer for GlutinFemtoVGRenderer {
    fn render(&self, _window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        self.renderer.render()
    }

    fn as_core_renderer(&self) -> &dyn Renderer {
        &self.renderer
    }
}
