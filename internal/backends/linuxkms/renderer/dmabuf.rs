// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The wgpu device behind the renderers that scan out a dma-buf.
//!
//! Every such renderer needs the same thing: a Vulkan device that can import a dma-buf
//! and release it to the display controller, opened to an application's configuration
//! where one was given.
//! See [`crate::display::gbmdmabufdisplay`] for the buffers that device renders into.

// cSpell: ignore dmabuf

use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::platform::PlatformError;
use wgpu_30 as wgpu;

/// Whether to take the dma-buf path without trying the wgpu DRM surface target first:
/// where `SLINT_KMS_WGPU_DMABUF` asks for it, or the surface target can't work.
pub fn prefer_dmabuf() -> bool {
    std::env::var_os("SLINT_KMS_WGPU_DMABUF").is_some() || !acquire_drm_display_available()
}

/// Whether the Vulkan loader offers the instance extension the wgpu DRM surface target needs.
/// Asking the loader avoids creating a Vulkan instance just for this.
///
/// A true here doesn't guarantee the surface target works: `vkAcquireDrmDisplayEXT`
/// can still fail for the specific DRM device, which is why the callers also fall
/// back on an error from surface creation.
fn acquire_drm_display_available() -> bool {
    // Safety: the loader is only asked for its extensions, and unloaded at the end of
    // this scope.
    unsafe {
        let Ok(entry) = ash::Entry::load() else { return false };
        entry.enumerate_instance_extension_properties(None).is_ok_and(|extensions| {
            extensions
                .iter()
                .any(|e| e.extension_name_as_c_str() == Ok(c"VK_EXT_acquire_drm_display"))
        })
    }
}

fn instance_descriptor() -> wgpu::InstanceDescriptor {
    wgpu::InstanceDescriptor {
        // Scanning out a dma-buf is a Vulkan-only path: the import needs
        // VK_EXT_external_memory_dma_buf. A `WGPUSettings::backends` asking for
        // anything else can't be honored here, and the other instance-level
        // settings and the power preference don't reach this path either; only
        // the device half of the configuration applies. Validation still follows
        // the usual environment variables through `InstanceFlags`.
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle_from_env()
    }
}

/// Requests an adapter of `instance`, as `WGPU_ADAPTER_NAME` or `WGPU_POWER_PREF` picks it.
fn request_adapter(instance: &wgpu::Instance) -> Result<wgpu::Adapter, wgpu::RequestAdapterError> {
    i_slint_core::graphics::wgpu_30::poll_once(wgpu::util::initialize_adapter_from_env_or_default(
        instance, None,
    ))
    .expect("internal error: wgpu setup is not expected to be async")
}

/// Blocks until everything submitted to the device's queue has executed.
///
/// `wgpu::Device::poll` only waits for wgpu's own submissions.
/// Skia draws through the raw queue and submits last, and so do the scanout barriers,
/// so a frame could reach the display before the GPU finished it.
/// Waiting on the queue itself covers every submitter.
pub fn wait_for_gpu(device: &wgpu::Device) -> Result<(), PlatformError> {
    // Safety: the queue is only waited on. Nothing else submits to it concurrently;
    // rendering is single-threaded.
    unsafe {
        let hal_device = vulkan_device(device)?;
        hal_device.raw_device().queue_wait_idle(hal_device.raw_queue())
    }
    .map_err(|e| format!("Error waiting for the GPU to finish the frame: {e}").into())
}

/// The Vulkan device behind `device`, for the Vulkan calls wgpu doesn't offer.
///
/// # Safety
/// The returned guard hands out raw Vulkan handles; see `wgpu::Device::as_hal`.
pub(crate) unsafe fn vulkan_device(
    device: &wgpu::Device,
) -> Result<impl std::ops::Deref<Target = wgpu::hal::vulkan::Device> + '_, PlatformError> {
    unsafe { device.as_hal::<wgpu::hal::api::Vulkan>() }
        .ok_or_else(|| PlatformError::from("The wgpu device is not a Vulkan device"))
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

    let adapter = request_adapter(&instance)
        .map_err(|e| format!("Error finding a Vulkan adapter for dma-buf rendering: {e}"))?;

    // The features and limits an application asked for, or everything the adapter
    // offers when it asked for nothing. i-slint-core reads them from the WGPU
    // configuration, whose type only exists with its `unstable-wgpu-30`, which
    // `renderer-femtovg-wgpu` doesn't enable by itself.
    let mut descriptor = i_slint_core::graphics::wgpu_30::surfaceless_device_descriptor(
        requested_graphics_api,
        &adapter,
    )?;
    if descriptor.label.is_none() {
        descriptor.label = Some("Slint linuxkms dma-buf device");
    }

    if !adapter.features().contains(wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF) {
        let display_access = if acquire_drm_display_available() {
            ""
        } else {
            ", and VK_EXT_acquire_drm_display, which direct display access needs"
        };
        return Err(PlatformError::Other(format!(
            "The Vulkan driver lacks VK_EXT_external_memory_dma_buf or \
             VK_EXT_image_drm_format_modifier, which rendering into a scanout buffer \
             needs{display_access}"
        )));
    }

    // Importing the scanout buffer needs this whatever the application asked for.
    descriptor.required_features |= wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF;

    let (device, queue) = open_device(&adapter, &descriptor)
        .map_err(|e| format!("Error creating a Vulkan device for dma-buf rendering: {e}"))?;

    Ok((instance, adapter, device, queue))
}

/// Opens the device with `VK_EXT_queue_family_foreign` where that's possible, see
/// [`open_device_with_queue_family_foreign`], and the ordinary way elsewhere.
fn open_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError> {
    open_device_with_queue_family_foreign(adapter, descriptor).unwrap_or_else(|| {
        i_slint_core::graphics::wgpu_30::poll_once(adapter.request_device(descriptor))
            .expect("internal error: wgpu setup is not expected to be async")
    })
}

/// Opens the device the way `request_device` would, with
/// `VK_EXT_queue_family_foreign` added.
///
/// Returns `None` when the extension can't be added, because the adapter doesn't support
/// it or isn't a Vulkan adapter after all.
/// The caller then opens the device the ordinary way, and releases scanout buffers to
/// `VK_QUEUE_FAMILY_EXTERNAL` instead.
/// Also returns `None` for a descriptor the adapter can't meet,
/// since opening the device this way skips the checks `request_device` reports those with.
fn open_device_with_queue_family_foreign(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Option<Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError>> {
    if !adapter.features().contains(descriptor.required_features)
        || descriptor.required_features.intersects(wgpu::Features::all_experimental_mask())
        || !descriptor.required_limits.check_limits(&adapter.limits())
    {
        return None;
    }
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

/// The queue family that a scanout buffer is released to, for the display controller.
///
/// `VK_QUEUE_FAMILY_FOREIGN_EXT` names a consumer outside Vulkan, which the display
/// controller is, but it needs [`QUEUE_FAMILY_FOREIGN`] enabled on the device.
/// Without it, `VK_QUEUE_FAMILY_EXTERNAL` is the closest core Vulkan offers.
pub fn scanout_queue_family_index(hal_device: &wgpu::hal::vulkan::Device) -> u32 {
    if hal_device.enabled_device_extensions().contains(&QUEUE_FAMILY_FOREIGN) {
        ash::vk::QUEUE_FAMILY_FOREIGN_EXT
    } else {
        ash::vk::QUEUE_FAMILY_EXTERNAL
    }
}

/// The device the dma-buf renderers get, with the validation layers loaded where they
/// are installed, for tests of the raw Vulkan work around the scanout buffers.
#[cfg(test)]
pub(crate) mod validation {
    use wgpu_30 as wgpu;

    /// Keeps every error wgpu logs, which is how wgpu-hal surfaces the validation
    /// layers' findings.
    struct ErrorRecorder(std::sync::Mutex<Vec<String>>);

    static RECORDER: ErrorRecorder = ErrorRecorder(std::sync::Mutex::new(Vec::new()));

    impl log::Log for ErrorRecorder {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Warn
        }
        fn log(&self, record: &log::Record) {
            eprintln!("[{} {}] {}", record.level(), record.target(), record.args());
            if record.level() == log::Level::Error {
                self.0.lock().unwrap().push(record.args().to_string());
            }
        }
        fn flush(&self) {}
    }

    /// Opens a device like [`super::init_wgpu`], minus the dma-buf import feature,
    /// which the tests don't need and a software driver may not have.
    /// Returns `None`, and says so, where there's no Vulkan adapter to test against.
    pub fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let _ = log::set_logger(&RECORDER).map(|()| log::set_max_level(log::LevelFilter::Warn));

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags: wgpu::InstanceFlags::VALIDATION | wgpu::InstanceFlags::DEBUG,
            backend_options: wgpu::BackendOptions::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            display: None,
        });
        let Ok(adapter) = super::request_adapter(&instance) else {
            eprintln!("No Vulkan adapter, so nothing to test against");
            return None;
        };
        eprintln!("Testing against {}", adapter.get_info().name);

        let descriptor =
            i_slint_core::graphics::wgpu_30::surfaceless_device_descriptor(None, &adapter)
                .expect("the default device descriptor");
        Some(super::open_device(&adapter, &descriptor).expect("creating the device"))
    }

    /// The errors wgpu logged so far, in any test.
    pub fn errors() -> Vec<String> {
        RECORDER.0.lock().unwrap().clone()
    }
}
