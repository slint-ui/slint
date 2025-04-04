// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;
use std::sync::Arc;

use i_slint_core::renderer::Renderer;
use i_slint_core::{graphics::RequestedGraphicsAPI, platform::PlatformError};
use i_slint_renderer_femtovg::{
    opengl, FemtoVGOpenGLRendererExt, FemtoVGRenderer, FemtoVGRendererExt,
};

use winit::event_loop::ActiveEventLoop;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

use super::WinitCompatibleRenderer;

#[cfg(not(target_arch = "wasm32"))]
mod glcontext;

pub struct GlutinFemtoVGRenderer {
    renderer: FemtoVGRenderer<opengl::OpenGLBackend>,
}

impl GlutinFemtoVGRenderer {
    pub fn new_suspended(
        _shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Box<dyn WinitCompatibleRenderer> {
        Box::new(Self { renderer: FemtoVGRenderer::new_suspended() })
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
        active_event_loop: &ActiveEventLoop,
        window_attributes: winit::window::WindowAttributes,
        #[cfg_attr(target_arch = "wasm32", allow(unused_variables))] requested_graphics_api: Option<
            RequestedGraphicsAPI,
        >,
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        let (winit_window, opengl_context) = glcontext::OpenGLContext::new_context(
            window_attributes,
            active_event_loop,
            requested_graphics_api.map(TryInto::try_into).transpose()?,
        )?;

        #[cfg(target_arch = "wasm32")]
        let winit_window = Arc::new(active_event_loop.create_window(window_attributes).map_err(
            |winit_os_error| {
                PlatformError::from(format!(
                    "FemtoVG Renderer: Could not create winit window wrapper for DOM canvas: {}",
                    winit_os_error
                ))
            },
        )?);

        self.renderer.set_opengl_context(
            #[cfg(not(target_arch = "wasm32"))]
            opengl_context,
            #[cfg(target_arch = "wasm32")]
            winit_window
                .canvas()
                .ok_or_else(|| "FemtoVG Renderer: winit didn't return a canvas")?,
        )?;

        Ok(winit_window)
    }

    fn suspend(&self) -> Result<(), PlatformError> {
        self.renderer.clear_graphics_context()
    }
}

#[cfg(all(feature = "renderer-femtovg-wgpu", not(target_family = "wasm")))]
pub struct WGPUFemtoVGRenderer {
    renderer: FemtoVGRenderer<i_slint_renderer_femtovg::wgpu::WGPUBackend>,
}

#[cfg(all(feature = "renderer-femtovg-wgpu", not(target_family = "wasm")))]
impl WGPUFemtoVGRenderer {
    pub fn new_suspended(
        _shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Box<dyn WinitCompatibleRenderer> {
        Box::new(Self {
            renderer: FemtoVGRenderer::<i_slint_renderer_femtovg::wgpu::WGPUBackend>::new_suspended(
            ),
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
        active_event_loop: &ActiveEventLoop,
        window_attributes: winit::window::WindowAttributes,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        let winit_window = Arc::new(active_event_loop.create_window(window_attributes).map_err(
            |winit_os_error| {
                PlatformError::from(format!(
                    "Error creating native window for FemtoVG rendering: {}",
                    winit_os_error
                ))
            },
        )?);

        let size = winit_window.inner_size();

        self.renderer.set_window_handle(
            Box::new(winit_window.clone()),
            crate::winitwindowadapter::physical_size_to_slint(&size),
            requested_graphics_api,
        )?;

        Ok(winit_window)
    }
}
