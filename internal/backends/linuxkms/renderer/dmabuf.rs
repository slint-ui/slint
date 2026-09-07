// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The wgpu device behind the renderers that scan out a dma-buf.
//!
//! Every such renderer needs the same thing: a Vulkan device that can import a
//! dma-buf and release it to the display controller, opened to an application's
//! configuration where one was given. See [`crate::display::gbmdmabufdisplay`]
//! for the buffers that device renders into.

// cSpell: ignore dmabuf

use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::platform::PlatformError;
use wgpu_30 as wgpu;

/// Whether the Vulkan loader offers the extension the wgpu DRM surface target
/// needs. wgpu enables it on the instance only when a driver advertises it, so
/// reading the enabled set back answers the question without creating a device.
///
/// A true here doesn't guarantee the surface target works: `vkAcquireDrmDisplayEXT`
/// can still fail for the specific DRM device, which is why the callers also fall
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
        // VK_EXT_external_memory_dma_buf. A `WGPUSettings::backends` asking for
        // anything else can't be honored here, and the other instance-level
        // settings don't reach this path either; only the device half of the
        // configuration applies. Validation still follows the usual environment
        // variables through `InstanceFlags`.
        backends: wgpu::Backends::VULKAN,
        flags: wgpu::InstanceFlags::from_build_config().with_env(),
        backend_options: wgpu::BackendOptions::from_env_or_default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        display: None,
    }
}

/// Blocks until everything submitted to the device's queue has executed.
///
/// `wgpu::Device::poll` only waits for wgpu's own submissions. Skia draws through
/// the raw queue and submits last, and the scanout barriers submit there too, so a
/// frame could reach the display before the GPU finished it. Waiting on the queue
/// itself covers every submitter.
pub fn wait_for_gpu(device: &wgpu::Device) -> Result<(), PlatformError> {
    // Safety: the queue is only waited on. Nothing else submits to it concurrently;
    // rendering is single-threaded.
    unsafe {
        let hal_device = device
            .as_hal::<wgpu::hal::api::Vulkan>()
            .ok_or_else(|| PlatformError::from("The wgpu device is not a Vulkan device"))?;
        hal_device.raw_device().queue_wait_idle(hal_device.raw_queue())
    }
    .map_err(|e| format!("Error waiting for the GPU to finish the frame: {e}").into())
}

/// The extension whose `VK_QUEUE_FAMILY_FOREIGN_EXT` names a consumer outside
/// Vulkan, which is what the display controller reading the dma-buf is. wgpu
/// doesn't ask for it on its own, so this path adds it to the device.
const QUEUE_FAMILY_FOREIGN: &std::ffi::CStr = c"VK_EXT_queue_family_foreign";

/// Creates the Vulkan-backed wgpu device the dma-buf renderers draw with.
pub fn init_wgpu(
    requested_graphics_api: Option<&RequestedGraphicsAPI>,
) -> Result<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue), PlatformError> {
    let instance = wgpu::Instance::new(instance_descriptor());

    let adapter = spin_on::spin_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::from_env().unwrap_or_default(),
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("Error finding a Vulkan adapter for dma-buf rendering: {e}"))?;

    // The features and limits an application asked for, or everything the adapter
    // offers when it asked for nothing. Naming the WGPU configuration here isn't
    // possible: its type only exists with i-slint-core's `unstable-wgpu-30`, which
    // `renderer-femtovg-wgpu` on its own doesn't enable.
    let mut descriptor = i_slint_core::graphics::wgpu_30::surfaceless_device_descriptor(
        requested_graphics_api,
        &adapter,
    )?;
    if descriptor.label.is_none() {
        descriptor.label = Some("Slint linuxkms dma-buf device");
    }
    // Importing the scanout buffer needs this whatever the application asked for.
    descriptor.required_features |= wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF;

    let (device, queue) = open_device_with_queue_family_foreign(&adapter, &descriptor)
        .unwrap_or_else(|| spin_on::spin_on(adapter.request_device(&descriptor)))
        .map_err(|e| format!("Error creating a Vulkan device for dma-buf rendering: {e}"))?;

    Ok((instance, adapter, device, queue))
}

/// Opens the device the way `request_device` would, with
/// `VK_EXT_queue_family_foreign` added.
///
/// Returns `None` when the extension can't be added — the adapter doesn't support
/// it, or it isn't a Vulkan adapter after all — leaving the caller to open the
/// device the ordinary way and release scanout buffers to `VK_QUEUE_FAMILY_EXTERNAL`
/// instead.
pub(crate) fn open_device_with_queue_family_foreign(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Option<Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError>> {
    // Safety: the hal adapter is only used to open a device, within this scope.
    let open_device = unsafe {
        let hal_adapter = adapter.as_hal::<wgpu::hal::api::Vulkan>()?;
        if !hal_adapter.physical_device_capabilities().supports_extension(QUEUE_FAMILY_FOREIGN) {
            return None;
        }
        hal_adapter.open_with_callback(
            descriptor.required_features,
            &descriptor.required_limits,
            &descriptor.memory_hints,
            Some(Box::new(|args| args.extensions.push(QUEUE_FAMILY_FOREIGN))),
        )
    }
    .inspect_err(|e| eprintln!("Error opening a Vulkan device with {QUEUE_FAMILY_FOREIGN:?}: {e}"))
    .ok()?;

    // Safety: `open_device` was just opened from this adapter with `descriptor`.
    Some(unsafe {
        adapter.create_device_from_hal::<wgpu::hal::api::Vulkan>(open_device, descriptor)
    })
}
