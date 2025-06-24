// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::{GraphicsAPI, PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::DirtyRegion;

use std::cell::RefCell;
use std::sync::Arc;

use wgpu_25 as wgpu;

use crate::SkiaSharedContext;

#[cfg(target_family = "windows")]
mod dx12;
#[cfg(target_vendor = "apple")]
mod metal;
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
mod vulkan;

/// This surface renders into the given window using Metal. The provided display argument
/// is ignored, as it has no meaning on macOS.
pub struct WGPUSurface {
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: RefCell<wgpu::SurfaceConfiguration>,
    surface: wgpu::Surface<'static>,
    textures_to_transition_for_sampling: RefCell<Vec<wgpu::Texture>>,
}

impl super::Surface for WGPUSurface {
    fn new(
        _shared_context: &SkiaSharedContext,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        let (instance, adapter, device, queue, surface) =
            i_slint_core::graphics::wgpu_25::init_instance_adapter_device_queue_surface(
                Box::new(WindowAndDisplayHandle(window_handle, display_handle)),
                requested_graphics_api,
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

        #[allow(unused_mut)]
        let mut gr_context: Option<skia_safe::gpu::DirectContext> = None;

        #[cfg(target_vendor = "apple")]
        {
            gr_context = metal::make_metal_context(&device, &queue);
        }

        #[cfg(target_family = "windows")]
        {
            gr_context = unsafe { dx12::make_dx12_context(&device, &queue) };
        }

        #[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
        {
            gr_context = unsafe { vulkan::make_vulkan_context(&device, &queue) };
        }

        Ok(Self {
            gr_context: RefCell::new(gr_context.ok_or_else(|| {
                i_slint_core::platform::PlatformError::from(
                    "Failed to create Skia context from WGPU",
                )
            })?),
            instance,
            device,
            queue,
            surface_config: surface_config.into(),
            surface,
            textures_to_transition_for_sampling: RefCell::new(Vec::new()),
        })
    }

    fn name(&self) -> &'static str {
        "wgpu"
    }

    fn resize_event(
        &self,
        size: PhysicalWindowSize,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let mut surface_config = self.surface_config.borrow_mut();

        surface_config.width = size.width;
        surface_config.height = size.height;

        self.surface.configure(&self.device, &surface_config);
        Ok(())
    }

    fn render(
        &self,
        _window: &Window,
        size: PhysicalWindowSize,
        callback: &dyn Fn(
            &skia_safe::Canvas,
            Option<&mut skia_safe::gpu::DirectContext>,
            u8,
        ) -> Option<DirtyRegion>,
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let gr_context = &mut self.gr_context.borrow_mut();

        let frame =
            self.surface.get_current_texture().expect("unable to get next texture from swapchain");

        #[allow(unused_mut)]
        let mut skia_surface: Option<skia_safe::Surface> = None;

        #[cfg(target_vendor = "apple")]
        {
            skia_surface = unsafe { metal::make_metal_surface(size, gr_context, &frame) };
        }

        #[cfg(target_family = "windows")]
        {
            skia_surface = unsafe { dx12::make_dx12_surface(size, gr_context, &frame) };
        }

        #[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
        {
            skia_surface = unsafe { self.make_vulkan_surface(size, gr_context, &frame) };
        }

        let mut skia_surface = skia_surface.ok_or_else(|| {
            i_slint_core::platform::PlatformError::from("Failed to Skia surface from WGPU")
        })?;

        callback(skia_surface.canvas(), Some(gr_context), 0);

        let textures_to_transition = self.textures_to_transition_for_sampling.take();
        if !textures_to_transition.is_empty() {
            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Skia texture transition encoder"),
            });
            encoder.transition_resources(
                std::iter::empty(),
                textures_to_transition.iter().map(|texture| wgpu::TextureTransition {
                    texture,
                    selector: None,
                    state: wgpu::TextureUses::RESOURCE,
                }),
            );

            self.queue.submit(Some(encoder.finish()));
        }

        gr_context.submit(None);

        frame.present();

        Ok(())
    }

    fn bits_per_pixel(&self) -> Result<u8, i_slint_core::platform::PlatformError> {
        //todo!()
        Ok(24)
    }

    fn supports_graphics_api() -> bool {
        true
    }

    fn supports_graphics_api_with_self(&self) -> bool {
        true
    }

    fn with_graphics_api(&self, callback: &mut dyn FnMut(GraphicsAPI<'_>)) {
        let api = i_slint_core::graphics::create_graphics_api_wgpu_25(
            self.instance.clone(),
            self.device.clone(),
            self.queue.clone(),
        );
        callback(api)
    }

    fn import_wgpu_texture(
        &self,
        canvas: &skia_safe::Canvas,
        any_wgpu_texture: &i_slint_core::graphics::WGPUTexture,
    ) -> Option<skia_safe::Image> {
        let texture = match any_wgpu_texture {
            i_slint_core::graphics::WGPUTexture::WGPU25Texture(texture) => texture.clone(),
        };

        // Skia won't submit commands right away, so remember the texture and transition before
        // submitting.
        self.textures_to_transition_for_sampling.borrow_mut().push(texture.clone());

        #[allow(unused_mut)]
        let mut image: Option<skia_safe::Image> = None;

        #[cfg(target_vendor = "apple")]
        {
            image = unsafe { metal::import_metal_texture(canvas, texture) };
        }

        #[cfg(target_family = "windows")]
        {
            image = unsafe { dx12::import_dx12_texture(canvas, texture) };
        }

        #[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
        {
            image = unsafe { vulkan::import_vulkan_texture(canvas, texture) };
        }

        image
    }
}

struct WindowAndDisplayHandle(
    Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
    Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
);

impl raw_window_handle::HasWindowHandle for WindowAndDisplayHandle {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.0.window_handle()
    }
}

impl raw_window_handle::HasDisplayHandle for WindowAndDisplayHandle {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.1.display_handle()
    }
}
