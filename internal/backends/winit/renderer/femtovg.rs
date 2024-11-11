// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::Cell;
use std::sync::Arc;

use i_slint_core::renderer::Renderer;
use i_slint_core::{graphics::RequestedGraphicsAPI, platform::PlatformError};
#[cfg(all(feature = "renderer-femtovg-wgpu", not(target_family = "wasm")))]
use i_slint_renderer_femtovg::FemtoVGRenderer;
use i_slint_renderer_femtovg::{
    FemtoVGOpenGLRenderer, FemtoVGOpenGLRendererExt, FemtoVGRendererExt,
};

#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

#[cfg(all(feature = "renderer-femtovg-wgpu", not(target_family = "wasm")))]
use crate::physical_size_to_slint;

use super::WinitCompatibleRenderer;

#[cfg(not(target_arch = "wasm32"))]
mod glcontext;

pub struct GlutinFemtoVGRenderer {
    renderer: FemtoVGOpenGLRenderer,
    suspended: Cell<bool>,
}

impl GlutinFemtoVGRenderer {
    pub fn new_suspended() -> Box<dyn WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: FemtoVGOpenGLRenderer::new_without_context(),
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
        window_attributes: winit::window::WindowAttributes,
        #[cfg_attr(target_arch = "wasm32", allow(unused_variables))] requested_graphics_api: Option<
            RequestedGraphicsAPI,
        >,
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        let (winit_window, opengl_context) = crate::event_loop::with_window_target(|event_loop| {
            Ok(glcontext::OpenGLContext::new_context(
                window_attributes,
                event_loop.event_loop(),
                requested_graphics_api.map(TryInto::try_into).transpose()?,
            )?)
        })?;

        #[cfg(target_arch = "wasm32")]
        let winit_window = Arc::new(crate::event_loop::with_window_target(|event_loop| {
            event_loop.create_window(window_attributes).map_err(|winit_os_error| {
                format!(
                    "FemtoVG Renderer: Could not create winit window wrapper for DOM canvas: {}",
                    winit_os_error
                )
                .into()
            })
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
        self.renderer.clear_graphics_context()
    }

    fn is_suspended(&self) -> bool {
        self.suspended.get()
    }
}

#[cfg(all(feature = "renderer-femtovg-wgpu", not(target_family = "wasm")))]
pub struct WGPUFemtoVGRenderer {
    renderer: FemtoVGRenderer<i_slint_renderer_femtovg::WGPUBackend>,
    suspended: Cell<bool>,
}

#[cfg(all(feature = "renderer-femtovg-wgpu", not(target_family = "wasm")))]
impl WGPUFemtoVGRenderer {
    pub fn new_suspended() -> Box<dyn WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: FemtoVGRenderer::<i_slint_renderer_femtovg::WGPUBackend>::new_without_context(
            ),
            suspended: Cell::new(true),
        })
    }
}

#[cfg(all(feature = "renderer-femtovg-wgpu", not(target_family = "wasm")))]
impl WinitCompatibleRenderer for WGPUFemtoVGRenderer {
    fn render(&self, _window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        self.renderer.render()
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn suspend(&self) -> Result<(), PlatformError> {
        self.renderer.clear_graphics_context()
    }

    fn resume(
        &self,
        window_attributes: winit::window::WindowAttributes,
        _requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        let winit_window = Arc::new(crate::event_loop::with_window_target(|event_loop| {
            event_loop.create_window(window_attributes).map_err(|winit_os_error| {
                format!("Error creating native window for Skia rendering: {}", winit_os_error)
                    .into()
            })
        })?);

        let size = winit_window.inner_size();

        self.renderer.backend().set_window_handle(
            &self.renderer,
            Box::new(winit_window.clone()),
            physical_size_to_slint(&size),
        )?;

        self.suspended.set(false);

        Ok(winit_window)
    }

    fn is_suspended(&self) -> bool {
        self.suspended.get()
    }
}
