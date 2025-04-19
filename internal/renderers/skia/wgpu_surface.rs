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
}

impl super::Surface for WGPUSurface {
    fn new(
        _shared_context: &SkiaSharedContext,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        let backends = wgpu::Backends::from_env().unwrap_or_default();
        let dx12_shader_compiler = wgpu::Dx12Compiler::from_env().unwrap_or_default();
        let gles_minor_version = wgpu::Gles3MinorVersion::from_env().unwrap_or_default();

        let instance = spin_on::spin_on(async {
            wgpu::util::new_instance_with_webgpu_detection(&wgpu::InstanceDescriptor {
                backends,
                flags: wgpu::InstanceFlags::from_build_config().with_env(),
                backend_options: wgpu::BackendOptions {
                    dx12: wgpu::Dx12BackendOptions { shader_compiler: dx12_shader_compiler },
                    gl: wgpu::GlBackendOptions { gles_minor_version, ..Default::default() },
                    noop: wgpu::NoopBackendOptions::default(),
                },
            })
            .await
        });

        let window_and_display_handle = WindowAndDisplayHandle(window_handle, display_handle);

        let surface = instance.create_surface(window_and_display_handle).unwrap();

        let adapter = spin_on::spin_on(async {
            wgpu::util::initialize_adapter_from_env_or_default(&instance, Some(&surface))
                .await
                .expect("Failed to find an appropriate adapter")
        });

        // HACK!!!
        let mut required_features = wgpu::Features::empty();
        let mut required_limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        required_features = wgpu::Features::PUSH_CONSTANTS;
        required_limits.max_push_constant_size = 16;

        let (device, queue) = spin_on::spin_on(async {
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: None,
                    required_features,
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    required_limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    trace: Default::default(),
                })
                .await
                .expect("Failed to create device")
        });

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
