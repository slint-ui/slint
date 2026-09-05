// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Skia on wgpu for drivers without `VK_EXT_acquire_drm_display`.
//!
//! The wgpu DRM surface target hands the display to Vulkan, which needs
//! `VK_EXT_acquire_drm_display`; few drivers have it. This path renders into a
//! GBM buffer imported as a wgpu texture instead, and posts it with a DRM page
//! flip. See [`crate::display::gbmdmabufdisplay`].

// cSpell: ignore dmabuf

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
use i_slint_renderer_skia::SkiaWGPU30Renderer;
use wgpu_30 as wgpu;

use crate::display::RenderingRotation;
use crate::display::gbmdmabufdisplay::GbmDmabufDisplay;
use crate::drmoutput::DrmOutput;

/// Whether the Vulkan loader offers the extension the wgpu DRM surface target
/// needs. wgpu enables it on the instance only when a driver advertises it, so
/// reading the enabled set back answers the question without creating a device.
///
/// A true here doesn't guarantee the surface target works: `vkAcquireDrmDisplayEXT`
/// can still fail for the specific DRM device, which is why the caller also falls
/// back on an error from surface creation.
pub fn acquire_drm_display_available() -> bool {
    let instance = wgpu::Instance::new(instance_descriptor());
    // Safety: the hal instance is only read from, and not kept beyond this scope.
    unsafe { instance.as_hal::<wgpu::hal::api::Vulkan>() }.is_some_and(|vulkan_instance| {
        vulkan_instance.shared_instance().extensions().contains(&c"VK_EXT_acquire_drm_display")
    })
}

fn instance_descriptor() -> wgpu::InstanceDescriptor {
    wgpu::InstanceDescriptor {
        // Scanning out a dma-buf is a Vulkan-only path: the import needs
        // VK_EXT_external_memory_dma_buf.
        backends: wgpu::Backends::VULKAN,
        flags: wgpu::InstanceFlags::from_build_config().with_env(),
        backend_options: wgpu::BackendOptions::from_env_or_default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        display: None,
    }
}

pub struct SkiaDmabufRendererAdapter {
    renderer: SkiaWGPU30Renderer,
    display: GbmDmabufDisplay,
    device: wgpu::Device,
    size: PhysicalWindowSize,
}

impl SkiaDmabufRendererAdapter {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        drm_output: DrmOutput,
        requested_graphics_api: Option<&RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        // Honoring a caller-provided wgpu configuration means matching on
        // `RequestedGraphicsAPI::WGPU30`, which only exists when i-slint-core has
        // its `unstable-wgpu-30` feature — something this crate can't rely on. A
        // surface-less init helper in i-slint-core would resolve that.
        match requested_graphics_api {
            None | Some(RequestedGraphicsAPI::Vulkan) => {}
            Some(api) => {
                return Err(PlatformError::Other(format!(
                    "Rendering into a dma-buf requires a Vulkan device created by the linuxkms \
                     backend, so the requested graphics API {api:?} can't be used"
                )));
            }
        }

        let (instance, adapter, device, queue) = init_wgpu()?;

        let (width, height) = drm_output.size();
        let size = PhysicalWindowSize::new(width, height);

        let display = GbmDmabufDisplay::new(drm_output, &device)?;

        let renderer = SkiaWGPU30Renderer::new(instance, adapter, device.clone(), queue)?;

        eprintln!("Using Skia renderer with wgpu, presenting dma-bufs on a DRM plane");

        Ok(Box::new(Self { renderer, display, device, size }))
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for SkiaDmabufRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<DrawOutcome, PlatformError> {
        // Make sure the buffer about to be rendered into is no longer the one on
        // its way to the screen.
        self.display.drm_output.wait_for_page_flip();

        self.renderer.render_to_scanout_texture(
            self.display.back_buffer(),
            rotation.degrees(),
            rotation.translation_after_rotation(self.size),
            Some(&|item_renderer| {
                draw_mouse_cursor_callback(item_renderer);
            }),
        )?;

        // KMS has no way to wait on the render: wgpu-hal exports no sync fd for
        // the plane's IN_FENCE_FD property, and wgpu won't take a signal semaphore
        // for a submission. Block until the GPU is done instead, at the cost of a
        // pipeline stall per frame.
        self.device
            .poll(wgpu::PollType::Wait { submission_index: None, timeout: None })
            .map_err(|e| format!("Error waiting for the GPU to finish the frame: {e}"))?;

        self.display.present().map_err(|e| format!("Error presenting dma-buf: {e}"))?;

        Ok(DrawOutcome::Success)
    }

    fn size(&self) -> PhysicalWindowSize {
        self.size
    }
}

fn init_wgpu() -> Result<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue), PlatformError>
{
    let instance = wgpu::Instance::new(instance_descriptor());

    let adapter = spin_on::spin_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::from_env().unwrap_or_default(),
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("Error finding a Vulkan adapter for dma-buf rendering: {e}"))?;

    let features = adapter.features() - wgpu::Features::all_experimental_mask();

    spin_on::spin_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("Slint linuxkms dma-buf device"),
        required_features: features,
        required_limits: adapter.limits(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::default(),
    }))
    .map(|(device, queue)| (instance, adapter, device, queue))
    .map_err(|e| format!("Error creating a Vulkan device for dma-buf rendering: {e}").into())
}
