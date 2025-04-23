// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::Cell;
use std::rc::Rc;

use i_slint_core::renderer::Renderer;
use i_slint_core::{graphics::RequestedGraphicsAPI, platform::PlatformError};
use i_slint_renderer_femtovg::{FemtoVGRenderer, FemtoVGRendererExt};

#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

use super::WinitCompatibleRenderer;

#[cfg(not(target_arch = "wasm32"))]
mod glcontext;

pub struct GlutinFemtoVGRenderer {
    renderer: FemtoVGRenderer,
    suspended: Cell<bool>,
}

impl GlutinFemtoVGRenderer {
    pub fn new_suspended() -> Box<dyn WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: FemtoVGRenderer::new_without_context(),
            suspended: Cell::new(true),
        })
    }
}

impl super::WinitCompatibleRenderer for GlutinFemtoVGRenderer {
    fn render(&self, _window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        self.renderer.render()
    }

    fn as_core_renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn resume(
        &self,
        event_loop: &dyn crate::event_loop::EventLoopInterface,
        window_attributes: winit::window::WindowAttributes,
        #[cfg_attr(target_arch = "wasm32", allow(unused_variables))] requested_graphics_api: Option<
            RequestedGraphicsAPI,
        >,
    ) -> Result<Rc<winit::window::Window>, PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        let (winit_window, opengl_context) = glcontext::OpenGLContext::new_context(
            window_attributes,
            event_loop.event_loop(),
            requested_graphics_api.map(TryInto::try_into).transpose()?,
        )?;

        #[cfg(target_arch = "wasm32")]
        let winit_window =
            Rc::new(event_loop.create_window(window_attributes).map_err(|winit_os_error| {
                PlatformError::from(format!(
                    "FemtoVG Renderer: Could not create winit window wrapper for DOM canvas: {}",
                    winit_os_error
                ))
            })?);

        self.renderer.set_opengl_context(
            #[cfg(not(target_arch = "wasm32"))]
            opengl_context,
            #[cfg(target_arch = "wasm32")]
            winit_window
                .canvas()
                .ok_or_else(|| "FemtoVG Renderer: winit didn't return a canvas")?,
        )?;

        self.suspended.set(false);

        Ok(winit_window)
    }

    fn suspend(&self) -> Result<(), PlatformError> {
        self.renderer.clear_opengl_context()
    }

    fn is_suspended(&self) -> bool {
        self.suspended.get()
    }
}
