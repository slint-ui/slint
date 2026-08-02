// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Delegate the rendering to vello.
//!
//! On the GPU that is [`i_slint_renderer_anyrender::VelloRenderer`] on WGPU;
//! when no GPU is available it is
//! [`i_slint_renderer_anyrender::VelloCpuRenderer`], which rasterizes on the
//! CPU and is presented through softbuffer like the software renderer.
//!
//! Which one it is, is decided when the renderer is created and stays fixed
//! for its lifetime: the window adapter hands out `&dyn Renderer` from the
//! moment it is constructed, so the core renderer cannot be exchanged later.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

#[cfg(all(feature = "renderer-vello", not(target_arch = "wasm32")))]
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::{DrawOutcome, Renderer};
#[cfg(feature = "renderer-vello-cpu")]
use i_slint_renderer_anyrender::VelloCpuRenderer;
#[cfg(all(feature = "renderer-vello-hybrid", target_arch = "wasm32"))]
use i_slint_renderer_anyrender::VelloHybridRenderer;
#[cfg(all(feature = "renderer-vello", not(target_arch = "wasm32")))]
use i_slint_renderer_anyrender::VelloRenderer;
use winit::event_loop::ActiveEventLoop;

use super::WinitCompatibleRenderer;

#[cfg(feature = "renderer-vello-cpu")]
type SoftbufferSurface =
    softbuffer::Surface<Arc<winit::window::Window>, Arc<winit::window::Window>>;

enum Backend {
    #[cfg(all(feature = "renderer-vello", not(target_arch = "wasm32")))]
    Gpu(Box<VelloRenderer>),
    /// In the browser vello_hybrid takes the place of the WGPU based vello:
    /// it renders through WebGL2, which vello's compute pipelines cannot use.
    #[cfg(all(feature = "renderer-vello-hybrid", target_arch = "wasm32"))]
    Hybrid(Box<VelloHybridRenderer>),
    #[cfg(feature = "renderer-vello-cpu")]
    Cpu {
        renderer: Box<VelloCpuRenderer>,
        // The context must outlive the surface, so it is dropped after it.
        surface: RefCell<Option<SoftbufferSurface>>,
        context: RefCell<Option<softbuffer::Context<Arc<winit::window::Window>>>>,
    },
}

impl Backend {
    #[cfg(feature = "renderer-vello-cpu")]
    fn cpu() -> Self {
        Self::Cpu {
            renderer: Box::new(VelloCpuRenderer::new_vello_cpu()),
            surface: RefCell::new(None),
            context: RefCell::new(None),
        }
    }
}

pub struct WinitVelloRenderer {
    backend: Backend,
    #[cfg(all(feature = "renderer-vello", not(target_arch = "wasm32")))]
    requested_graphics_api: Option<RequestedGraphicsAPI>,
}

impl WinitVelloRenderer {
    /// Render with vello on the GPU, falling back to vello_cpu when WGPU
    /// reports no usable adapter.
    #[cfg(all(feature = "renderer-vello-hybrid", target_arch = "wasm32"))]
    pub fn new_suspended(
        _shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self { backend: Backend::Hybrid(Box::new(VelloHybridRenderer::new_vello_hybrid())) }))
    }

    #[cfg(all(feature = "renderer-vello", not(target_arch = "wasm32")))]
    pub fn new_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn WinitCompatibleRenderer>, PlatformError> {
        let requested_graphics_api = shared_backend_data.requested_graphics_api.clone();

        // The adapter probe is the last point where the fallback can still be
        // chosen; a failure later, when the surface is created, can only be
        // reported as an error.
        let backend = if i_slint_core::graphics::wgpu_29::any_wgpu29_adapters_with_gpu(
            requested_graphics_api.clone(),
        ) {
            Backend::Gpu(Box::new(VelloRenderer::new_vello()))
        } else if requested_graphics_api.is_some() {
            // The application asked for a specific WGPU device, so silently
            // rasterizing on the CPU instead would ignore that request.
            return Err(PlatformError::from(
                "WGPU: No GPU adapters found for the requested graphics API",
            ));
        } else {
            Backend::cpu()
        };

        Ok(Box::new(Self { backend, requested_graphics_api }))
    }

    /// Rasterize with vello_cpu, without looking for a GPU at all.
    #[cfg(feature = "renderer-vello-cpu")]
    pub fn new_cpu_suspended(
        _shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn WinitCompatibleRenderer>, PlatformError> {
        Ok(Box::new(Self {
            backend: Backend::cpu(),
            #[cfg(all(feature = "renderer-vello", not(target_arch = "wasm32")))]
            requested_graphics_api: None,
        }))
    }

    /// Copy the rasterized frame into the softbuffer surface and present it.
    #[cfg(feature = "renderer-vello-cpu")]
    fn present_cpu_frame(
        renderer: &VelloCpuRenderer,
        surface: &RefCell<Option<SoftbufferSurface>>,
        window: &i_slint_core::api::Window,
    ) -> Result<(), PlatformError> {
        let size = window.size();
        let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok())
        else {
            return Ok(());
        };

        let mut borrowed_surface = surface.borrow_mut();
        let Some(surface) = borrowed_surface.as_mut() else {
            return Ok(());
        };

        surface
            .resize(width, height)
            .map_err(|e| format!("Error resizing softbuffer surface: {e}"))?;
        let winit_window = surface.window().clone();
        let mut target_buffer = surface
            .buffer_mut()
            .map_err(|e| format!("Error retrieving softbuffer rendering buffer: {e}"))?;

        renderer.with_frame_buffer(|buffer, buffer_width, buffer_height| {
            let rows = buffer_height.min(height.get()) as usize;
            let columns = buffer_width.min(width.get()) as usize;
            for row in 0..rows {
                let source = &buffer[row * buffer_width as usize * 4..];
                let destination = &mut target_buffer[row * width.get() as usize..];
                for column in 0..columns {
                    let pixel = &source[column * 4..column * 4 + 4];
                    // softbuffer expects 0RGB; vello_cpu's alpha is
                    // premultiplied, which for the opaque window background is
                    // the same as the straight color.
                    destination[column] = ((pixel[0] as u32) << 16)
                        | ((pixel[1] as u32) << 8)
                        | (pixel[2] as u32);
                }
            }
        });

        winit_window.pre_present_notify();
        target_buffer.present().map_err(|e| format!("Error presenting softbuffer buffer: {e}"))?;
        Ok(())
    }
}

impl WinitCompatibleRenderer for WinitVelloRenderer {
    fn render(&self, _window: &i_slint_core::api::Window) -> Result<DrawOutcome, PlatformError> {
        match &self.backend {
            #[cfg(all(feature = "renderer-vello", not(target_arch = "wasm32")))]
            Backend::Gpu(renderer) => renderer.render()?,
            #[cfg(all(feature = "renderer-vello-hybrid", target_arch = "wasm32"))]
            Backend::Hybrid(renderer) => renderer.render()?,
            #[cfg(feature = "renderer-vello-cpu")]
            Backend::Cpu { renderer, surface, .. } => {
                renderer.render()?;
                Self::present_cpu_frame(renderer, surface, _window)?;
            }
        }
        // vello submits the whole scene every frame, so there is no partially
        // rendered outcome to report.
        Ok(DrawOutcome::Success)
    }

    fn as_core_renderer(&self) -> &dyn Renderer {
        match &self.backend {
            #[cfg(all(feature = "renderer-vello", not(target_arch = "wasm32")))]
            Backend::Gpu(renderer) => renderer.as_ref(),
            #[cfg(all(feature = "renderer-vello-hybrid", target_arch = "wasm32"))]
            Backend::Hybrid(renderer) => renderer.as_ref(),
            #[cfg(feature = "renderer-vello-cpu")]
            Backend::Cpu { renderer, .. } => renderer.as_ref(),
        }
    }

    fn suspend(&self) -> Result<(), PlatformError> {
        match &self.backend {
            #[cfg(all(feature = "renderer-vello", not(target_arch = "wasm32")))]
            Backend::Gpu(renderer) => {
                // Also releases the winit window the callback holds on to.
                renderer.set_pre_present_callback(None);
                renderer.suspend_window();
            }
            #[cfg(all(feature = "renderer-vello-hybrid", target_arch = "wasm32"))]
            Backend::Hybrid(renderer) => renderer.clear_canvas(),
            #[cfg(feature = "renderer-vello-cpu")]
            Backend::Cpu { surface, context, .. } => {
                drop(surface.borrow_mut().take());
                drop(context.borrow_mut().take());
            }
        }
        Ok(())
    }

    fn resume(
        &self,
        active_event_loop: &ActiveEventLoop,
        window_attributes: winit::window::WindowAttributes,
        _window_adapter_weak: std::rc::Weak<crate::winitwindowadapter::WinitWindowAdapter>,
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        let _transparent = window_attributes.transparent;

        let winit_window = Arc::new(active_event_loop.create_window(window_attributes).map_err(
            |winit_os_error| {
                PlatformError::from(format!(
                    "Error creating native window for vello rendering: {winit_os_error}"
                ))
            },
        )?);

        let size = winit_window.inner_size();

        match &self.backend {
            #[cfg(all(feature = "renderer-vello", not(target_arch = "wasm32")))]
            Backend::Gpu(renderer) => {
                renderer.resume_window(
                    winit_window.clone(),
                    size.width.max(1),
                    size.height.max(1),
                    _transparent,
                    self.requested_graphics_api.clone(),
                )?;

                renderer.set_pre_present_callback(Some(Box::new({
                    let winit_window = winit_window.clone();
                    move || {
                        winit_window.pre_present_notify();
                    }
                })));
            }
            #[cfg(all(feature = "renderer-vello-hybrid", target_arch = "wasm32"))]
            Backend::Hybrid(renderer) => {
                use winit::platform::web::WindowExtWebSys;
                let canvas = winit_window
                    .canvas()
                    .ok_or_else(|| PlatformError::from("vello_hybrid: winit did not return a canvas"))?;
                renderer.set_canvas(canvas, size.width.max(1), size.height.max(1))?;
            }
            #[cfg(feature = "renderer-vello-cpu")]
            Backend::Cpu { renderer, surface, context } => {
                let softbuffer_context = softbuffer::Context::new(winit_window.clone())
                    .map_err(|e| format!("Error creating softbuffer context: {e}"))?;
                let softbuffer_surface =
                    softbuffer::Surface::new(&softbuffer_context, winit_window.clone())
                        .map_err(|e| format!("Error creating softbuffer surface: {e}"))?;

                renderer.set_surface_size(size.width.max(1), size.height.max(1));

                *context.borrow_mut() = Some(softbuffer_context);
                *surface.borrow_mut() = Some(softbuffer_surface);
            }
        }

        Ok(winit_window)
    }
}
