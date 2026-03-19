// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;
#[cfg(supports_opengl)]
use std::rc::Weak;
use std::sync::Arc;

use i_slint_core::renderer::Renderer;
use i_slint_core::{graphics::RequestedGraphicsAPI, platform::PlatformError};
#[cfg(supports_opengl)]
use i_slint_renderer_femtovg::{FemtoVGOpenGLRendererExt, opengl};
use i_slint_renderer_femtovg::{FemtoVGRenderer, FemtoVGRendererExt};

use winit::event_loop::ActiveEventLoop;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

use super::WinitCompatibleRenderer;

#[cfg(all(supports_opengl, not(target_arch = "wasm32")))]
mod glcontext;

#[cfg(supports_opengl)]
pub struct GlutinFemtoVGRenderer {
    renderer: FemtoVGRenderer<opengl::OpenGLBackend>,
    _requested_graphics_api: Option<RequestedGraphicsAPI>,
    _shared_backend_data_weak: Weak<crate::SharedBackendData>,
}

#[cfg(supports_opengl)]
impl GlutinFemtoVGRenderer {
    pub fn new_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            renderer: FemtoVGRenderer::new_suspended(),
            _requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
            _shared_backend_data_weak: Rc::downgrade(shared_backend_data),
        }))
    }
}

#[cfg(supports_opengl)]
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
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        let (winit_window, opengl_context) = glcontext::OpenGLContext::new_context(
            window_attributes,
            active_event_loop,
            self._requested_graphics_api.as_ref().map(TryInto::try_into).transpose()?,
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

        #[cfg(target_family = "wasm")]
        let html_canvas = winit_window
            .canvas()
            .ok_or_else(|| "FemtoVG Renderer: winit didn't return a canvas")?;

        self.renderer.set_opengl_context(
            #[cfg(not(target_arch = "wasm32"))]
            opengl_context,
            #[cfg(target_arch = "wasm32")]
            html_canvas.clone(),
        )?;

        #[cfg(target_family = "wasm")]
        self.setup_webgl_context_loss_handlers(winit_window.id(), html_canvas);

        Ok(winit_window)
    }

    fn suspend(&self) -> Result<(), PlatformError> {
        self.renderer.clear_graphics_context()
    }
}

#[cfg(all(supports_opengl, target_family = "wasm"))]
impl GlutinFemtoVGRenderer {
    fn setup_webgl_context_loss_handlers(
        &self,
        window_id: winit::window::WindowId,
        html_canvas: web_sys::HtmlCanvasElement,
    ) {
        use wasm_bindgen::JsCast;
        use wasm_bindgen::closure::Closure;

        let add_listener = |name, closure: Closure<dyn Fn(web_sys::WebGlContextEvent)>| {
            html_canvas
                .add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())
                .unwrap();
            closure.forget();
        };

        add_listener(
            "webglcontextlost",
            Closure::wrap(Box::new({
                let shared_backend_data_weak = self._shared_backend_data_weak.clone();
                move |event: web_sys::WebGlContextEvent| {
                    let Some(window_adapter) = shared_backend_data_weak
                        .upgrade()
                        .and_then(|backend_data| backend_data.window_by_id(window_id))
                    else {
                        return;
                    };
                    i_slint_core::debug_log!(
                        "Slint: Suspending renderer due to WebGL context loss"
                    );
                    let this = (window_adapter.renderer() as &dyn std::any::Any)
                        .downcast_ref::<Self>()
                        .unwrap();
                    let _ = this.renderer.clear_graphics_context().ok();
                    // Preventing default is the way to make sure the browser sends a webglcontextrestored event
                    // when the context is back.
                    event.prevent_default();
                }
            }) as Box<dyn Fn(web_sys::WebGlContextEvent)>),
        );
        add_listener(
            "webglcontextrestored",
            Closure::wrap(Box::new({
                let shared_backend_data_weak = self._shared_backend_data_weak.clone();
                let html_canvas = html_canvas.clone();
                move |_event: web_sys::WebGlContextEvent| {
                    let Some(window_adapter) = shared_backend_data_weak
                        .upgrade()
                        .and_then(|backend_data| backend_data.window_by_id(window_id))
                    else {
                        return;
                    };
                    i_slint_core::debug_log!(
                        "Slint: Restoring renderer due to WebGL context restoration"
                    );
                    let this = (window_adapter.renderer() as &dyn std::any::Any)
                        .downcast_ref::<Self>()
                        .unwrap();
                    if this.renderer.set_opengl_context(html_canvas.clone()).is_ok() {
                        use i_slint_core::platform::WindowAdapter;
                        window_adapter.request_redraw();
                        let _ = window_adapter.draw().ok();
                    }
                }
            }) as Box<dyn Fn(web_sys::WebGlContextEvent)>),
        );
    }
}

#[cfg(all(feature = "renderer-femtovg-wgpu", not(target_family = "wasm")))]
pub struct WGPUFemtoVGRenderer {
    renderer: FemtoVGRenderer<i_slint_renderer_femtovg::wgpu::WGPUBackend>,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
}

#[cfg(all(feature = "renderer-femtovg-wgpu", not(target_family = "wasm")))]
impl WGPUFemtoVGRenderer {
    pub fn new_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn WinitCompatibleRenderer>, PlatformError> {
        if !i_slint_core::graphics::wgpu_28::any_wgpu28_adapters_with_gpu(
            shared_backend_data._requested_graphics_api.clone(),
        ) {
            return Err(PlatformError::from("WGPU: No GPU adapters found"));
        }
        Ok(Box::new(Self {
            renderer: FemtoVGRenderer::<i_slint_renderer_femtovg::wgpu::WGPUBackend>::new_suspended(
            ),
            requested_graphics_api: shared_backend_data._requested_graphics_api.clone(),
        }))
    }
}

#[cfg(all(feature = "renderer-femtovg-wgpu", not(target_family = "wasm")))]
impl WinitCompatibleRenderer for WGPUFemtoVGRenderer {
    fn render(&self, _window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        self.renderer.render()
    }

    fn as_core_renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn suspend(&self) -> Result<(), PlatformError> {
        self.renderer.clear_graphics_context()
    }

    fn resume(
        &self,
        active_event_loop: &ActiveEventLoop,
        window_attributes: winit::window::WindowAttributes,
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
            self.requested_graphics_api.clone(),
        )?;

        Ok(winit_window)
    }
}
