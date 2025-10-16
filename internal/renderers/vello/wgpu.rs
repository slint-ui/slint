// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{cell::RefCell, rc::Rc};

use i_slint_core::{api::PhysicalSize as PhysicalWindowSize, graphics::RequestedGraphicsAPI};

use crate::{FemtoVGRenderer, GraphicsBackend, WindowSurface};

use wgpu_27 as wgpu;

pub struct WGPUBackend {
    instance: RefCell<Option<wgpu::Instance>>,
    device: RefCell<Option<wgpu::Device>>,
    queue: RefCell<Option<wgpu::Queue>>,
    surface_config: RefCell<Option<wgpu::SurfaceConfiguration>>,
    surface: RefCell<Option<wgpu::Surface<'static>>>,
}

pub struct WGPUWindowSurface {
    surface_texture: wgpu::SurfaceTexture,
}

impl WindowSurface<femtovg::renderer::WGPURenderer> for WGPUWindowSurface {
    fn render_surface(&self) -> &wgpu::Texture {
        &self.surface_texture.texture
    }
}

impl GraphicsBackend for WGPUBackend {
    type Renderer = femtovg::renderer::WGPURenderer;
    type WindowSurface = WGPUWindowSurface;
    const NAME: &'static str = "WGPU";

    fn new_suspended() -> Self {
        Self {
            instance: Default::default(),
            device: Default::default(),
            queue: Default::default(),
            surface_config: Default::default(),
            surface: Default::default(),
        }
    }

    fn clear_graphics_context(&self) {
        self.surface.borrow_mut().take();
        self.queue.borrow_mut().take();
        self.device.borrow_mut().take();
    }

    fn begin_surface_rendering(
        &self,
    ) -> Result<Self::WindowSurface, Box<dyn std::error::Error + Send + Sync>> {
        let frame = self
            .surface
            .borrow()
            .as_ref()
            .unwrap()
            .get_current_texture()
            .expect("unable to get next texture from swapchain");
        Ok(WGPUWindowSurface { surface_texture: frame })
    }

    fn submit_commands(&self, commands: <Self::Renderer as femtovg::Renderer>::CommandBuffer) {
        self.queue.borrow().as_ref().unwrap().submit(Some(commands));
    }

    fn present_surface(
        &self,
        surface: Self::WindowSurface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        surface.surface_texture.present();
        Ok(())
    }

    #[cfg(feature = "unstable-wgpu-27")]
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        let instance = self.instance.borrow().clone();
        let device = self.device.borrow().clone();
        let queue = self.queue.borrow().clone();
        if let (Some(instance), Some(device), Some(queue)) = (instance, device, queue) {
            Ok(callback(Some(i_slint_core::graphics::create_graphics_api_wgpu_27(
                instance, device, queue,
            ))))
        } else {
            Ok(callback(None))
        }
    }

    #[cfg(not(feature = "unstable-wgpu-27"))]
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        Ok(callback(None))
    }

    fn resize(
        &self,
        width: std::num::NonZeroU32,
        height: std::num::NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut surface_config = self.surface_config.borrow_mut();
        let Some(surface_config) = surface_config.as_mut() else {
            // When the backend dispatches a resize event while the renderer is suspended, ignore resize requests.
            return Ok(());
        };

        // Prefer FIFO modes over possible Mailbox setting for frame pacing and better energy efficiency.
        surface_config.present_mode = wgpu::PresentMode::AutoVsync;
        surface_config.width = width.get();
        surface_config.height = height.get();

        let mut device = self.device.borrow_mut();
        let device = device.as_mut().unwrap();

        self.surface.borrow_mut().as_mut().unwrap().configure(device, surface_config);
        Ok(())
    }
}

impl FemtoVGRenderer<WGPUBackend> {
    pub fn set_window_handle(
        &self,
        window_handle: Box<dyn wgpu::WindowHandle>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (instance, adapter, device, queue, surface) =
            i_slint_core::graphics::wgpu_27::init_instance_adapter_device_queue_surface(
                window_handle,
                requested_graphics_api,
                /* rendering artifacts :( */
                wgpu::Backends::GL,
            )?;

        let mut surface_config =
            surface.get_default_config(&adapter, size.width, size.height).unwrap();

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or_else(|| swapchain_capabilities.formats[0]);
        surface_config.format = swapchain_format;
        surface.configure(&device, &surface_config);

        *self.graphics_backend.instance.borrow_mut() = Some(instance.clone());
        *self.graphics_backend.device.borrow_mut() = Some(device.clone());
        *self.graphics_backend.queue.borrow_mut() = Some(queue.clone());
        *self.graphics_backend.surface_config.borrow_mut() = Some(surface_config);
        *self.graphics_backend.surface.borrow_mut() = Some(surface);

        let wgpu_renderer = femtovg::renderer::WGPURenderer::new(device, queue);
        let femtovg_canvas = femtovg::Canvas::new_with_text_context(
            wgpu_renderer,
            crate::font_cache::FONT_CACHE.with(|cache| cache.borrow().text_context.clone()),
        )
        .unwrap();

        let canvas = Rc::new(RefCell::new(femtovg_canvas));
        self.reset_canvas(canvas);
        Ok(())
    }
}
