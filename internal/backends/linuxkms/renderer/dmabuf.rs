// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The wgpu device behind the renderers that scan out a dma-buf.
//!
//! Every such renderer needs the same thing: a Vulkan device that can import a dma-buf
//! and release it to the display controller, opened to an application's configuration
//! where one was given.
//! See [`crate::display::gbmdmabufdisplay`] for the buffers that device renders into.

// cSpell: ignore dmabuf lavapipe

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

/// The extension that exports a semaphore as a sync file, see `ScanoutBarriers::release`.
/// KMS waits for the sync file before it flips the frame.
/// wgpu doesn't ask for it on its own either.
pub const EXTERNAL_SEMAPHORE_FD: &std::ffi::CStr = c"VK_KHR_external_semaphore_fd";

/// Creates the Vulkan-backed wgpu device the dma-buf renderers draw with.
pub fn init_wgpu(
    requested_graphics_api: Option<&RequestedGraphicsAPI>,
) -> Result<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue), PlatformError> {
    let instance = wgpu::Instance::new(instance_descriptor());

    let adapter = request_adapter(&instance)
        .map_err(|e| format!("Error finding a Vulkan adapter for dma-buf rendering: {e}"))?;
    // A software Vulkan driver such as lavapipe is installed alongside GPUs whose only
    // driver is OpenGL. Rendering through it would be slower than the OpenGL and
    // software renderers that come next.
    let info = adapter.get_info();
    if info.device_type == wgpu::DeviceType::Cpu && std::env::var_os("SLINT_WGPU_CPU").is_none() {
        return Err(PlatformError::Other(format!(
            "The Vulkan adapter for dma-buf rendering, {}, renders on the CPU. \
             Set SLINT_WGPU_CPU to use it anyway",
            info.name
        )));
    }

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

/// Opens the device with [`QUEUE_FAMILY_FOREIGN`] and [`EXTERNAL_SEMAPHORE_FD`] where
/// that's possible, see [`open_device_with_extensions`], and the ordinary way elsewhere.
fn open_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError> {
    open_device_with_extensions(adapter, descriptor).unwrap_or_else(|| {
        i_slint_core::graphics::wgpu_30::poll_once(adapter.request_device(descriptor))
            .expect("internal error: wgpu setup is not expected to be async")
    })
}

/// Opens the device the way `request_device` would,
/// adding [`QUEUE_FAMILY_FOREIGN`] and [`EXTERNAL_SEMAPHORE_FD`] where the adapter supports them.
///
/// Returns `None` when neither can be added, or the adapter isn't a Vulkan adapter after all.
/// The caller then opens the device the ordinary way.
/// Without [`QUEUE_FAMILY_FOREIGN`], scanout buffers are released to `VK_QUEUE_FAMILY_EXTERNAL`.
/// Without [`EXTERNAL_SEMAPHORE_FD`], the backend waits for the GPU before posting a frame.
/// Also returns `None` for a descriptor the adapter can't meet,
/// since opening the device this way skips the checks `request_device` reports those with.
fn open_device_with_extensions(
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
        let capabilities = hal_adapter.physical_device_capabilities();
        let foreign =
            capabilities.supports_extension(QUEUE_FAMILY_FOREIGN).then_some(QUEUE_FAMILY_FOREIGN);
        let sync_files = (cfg!(skia_wgpu_30)
            && capabilities.supports_extension(EXTERNAL_SEMAPHORE_FD)
            && exports_sync_file_semaphores(&hal_adapter))
        .then_some(EXTERNAL_SEMAPHORE_FD);
        let open = |extensions: Vec<&'static std::ffi::CStr>| {
            hal_adapter
                .open_with_callback(
                    descriptor.required_features,
                    &descriptor.required_limits,
                    &descriptor.memory_hints,
                    Some(Box::new({
                        let extensions = extensions.clone();
                        move |args| args.extensions.extend(extensions)
                    })),
                )
                .inspect_err(|e| {
                    eprintln!("Error opening a Vulkan device with {extensions:?}: {e}")
                })
                .ok()
        };
        sync_files
            .and_then(|sync_files| open(foreign.into_iter().chain([sync_files]).collect()))
            .or_else(|| foreign.and_then(|foreign| open(vec![foreign])))
    }?;

    // Safety: `open_device` was just opened from this adapter with `descriptor`.
    Some(unsafe {
        adapter.create_device_from_hal::<wgpu::hal::api::Vulkan>(open_device, descriptor)
    })
}

/// Drivers can offer [`EXTERNAL_SEMAPHORE_FD`] for opaque file descriptors only.
/// Needs Vulkan 1.1, where external semaphores, the extension's prerequisite, are core.
///
/// # Safety
///
/// The instance behind `hal_adapter` must be alive.
unsafe fn exports_sync_file_semaphores(hal_adapter: &wgpu::hal::vulkan::Adapter) -> bool {
    let instance = hal_adapter.shared_instance();
    if instance.instance_api_version() < ash::vk::API_VERSION_1_1 {
        return false;
    }
    // Safety: the physical device belongs to the instance, which outlives this call.
    let device = unsafe {
        instance.raw_instance().get_physical_device_properties(hal_adapter.raw_physical_device())
    };
    if device.api_version < ash::vk::API_VERSION_1_1 {
        return false;
    }
    let info = ash::vk::PhysicalDeviceExternalSemaphoreInfo::default()
        .handle_type(ash::vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD);
    let mut properties = ash::vk::ExternalSemaphoreProperties::default();
    // Safety: as above.
    unsafe {
        instance.raw_instance().get_physical_device_external_semaphore_properties(
            hal_adapter.raw_physical_device(),
            &info,
            &mut properties,
        );
    }
    properties
        .external_semaphore_features
        .contains(ash::vk::ExternalSemaphoreFeatureFlags::EXPORTABLE)
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
