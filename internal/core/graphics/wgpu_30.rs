// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]

/*!
This module contains types that are public and re-exported in the slint-rs as well as the slint-interpreter crate as public API,
in particular the `BackendSelector` type, to configure the WGPU-based renderer(s).
*/

use alloc::boxed::Box;

pub use wgpu_30 as wgpu;

#[cfg(feature = "unstable-wgpu-30")]
pub mod api {
    /*!
    This module contains types that are public and re-exported in the slint-rs as well as the slint-interpreter crate as public API.
    */

    #[doc(no_inline)]
    pub use super::wgpu;

    /// This data structure provides settings for initializing WGPU renderers.
    #[derive(Clone, Debug)]
    pub struct WGPUSettings {
        /// The backends to use for the WGPU instance.
        pub backends: wgpu_30::Backends,
        /// The different options that are given to the selected backends.
        pub backend_options: wgpu_30::BackendOptions,
        /// The flags to fine-tune behavior of the WGPU instance.
        pub instance_flags: wgpu_30::InstanceFlags,
        /// Memory budget thresholds used by some backends.
        pub instance_memory_budget_thresholds: wgpu_30::MemoryBudgetThresholds,

        /// The power preference is used to influence the WGPU adapter selection.
        pub power_preference: wgpu_30::PowerPreference,

        /// The label for the device. This is used to identify the device in debugging tools.
        pub device_label: Option<std::borrow::Cow<'static, str>>,
        /// The required features for the device.
        pub device_required_features: wgpu_30::Features,
        /// The required limits for the device.
        pub device_required_limits: wgpu_30::Limits,
        /// The experimental features for the device.
        pub device_experimental_features: wgpu_30::ExperimentalFeatures,
        /// The memory hints for the device.
        pub device_memory_hints: wgpu_30::MemoryHints,
    }

    impl Default for WGPUSettings {
        fn default() -> Self {
            let backends = wgpu_30::Backends::from_env().unwrap_or_default();

            Self {
                backends,
                backend_options: wgpu_30::BackendOptions::from_env_or_default(),
                instance_flags: wgpu_30::InstanceFlags::from_build_config().with_env(),
                instance_memory_budget_thresholds: wgpu_30::MemoryBudgetThresholds::default(),

                power_preference: wgpu_30::PowerPreference::from_env().unwrap_or_default(),

                device_label: None,
                device_required_features: wgpu_30::Features::empty(),
                device_required_limits: wgpu_30::Limits::downlevel_webgl2_defaults(),
                device_experimental_features: wgpu_30::ExperimentalFeatures::disabled(),
                device_memory_hints: wgpu_30::MemoryHints::MemoryUsage,
            }
        }
    }

    /// Compares every field. Several of the nested wgpu types don't implement `PartialEq`
    /// themselves; those are compared field by field through the helpers below. Used to
    /// decide whether a WGPU device created with one settings value can be shared with a
    /// renderer requesting another.
    impl PartialEq for WGPUSettings {
        fn eq(&self, other: &Self) -> bool {
            let Self {
                backends,
                backend_options,
                instance_flags,
                instance_memory_budget_thresholds,
                power_preference,
                device_label,
                device_required_features,
                device_required_limits,
                device_experimental_features,
                device_memory_hints,
            } = self;
            *backends == other.backends
                && backend_options_eq(backend_options, &other.backend_options)
                && *instance_flags == other.instance_flags
                && memory_budget_thresholds_eq(
                    instance_memory_budget_thresholds,
                    &other.instance_memory_budget_thresholds,
                )
                && *power_preference == other.power_preference
                && *device_label == other.device_label
                && *device_required_features == other.device_required_features
                && *device_required_limits == other.device_required_limits
                && device_experimental_features.is_enabled()
                    == other.device_experimental_features.is_enabled()
                && memory_hints_eq(device_memory_hints, &other.device_memory_hints)
        }
    }

    fn backend_options_eq(a: &wgpu_30::BackendOptions, b: &wgpu_30::BackendOptions) -> bool {
        let wgpu_30::BackendOptions { gl, dx12, noop } = a;
        gl_backend_options_eq(gl, &b.gl)
            && dx12_backend_options_eq(dx12, &b.dx12)
            && noop_backend_options_eq(noop, &b.noop)
    }

    fn gl_backend_options_eq(a: &wgpu_30::GlBackendOptions, b: &wgpu_30::GlBackendOptions) -> bool {
        let wgpu_30::GlBackendOptions { gles_minor_version, fence_behavior, debug_fns } = a;
        *gles_minor_version == b.gles_minor_version
            && *fence_behavior == b.fence_behavior
            && *debug_fns == b.debug_fns
    }

    fn dx12_backend_options_eq(
        a: &wgpu_30::Dx12BackendOptions,
        b: &wgpu_30::Dx12BackendOptions,
    ) -> bool {
        let wgpu_30::Dx12BackendOptions {
            shader_compiler,
            presentation_system,
            latency_waitable_object,
            force_shader_model,
            agility_sdk,
        } = a;
        dx12_compiler_eq(shader_compiler, &b.shader_compiler)
            && *presentation_system == b.presentation_system
            // Fieldless enum without `PartialEq`; the discriminant is the whole value.
            && core::mem::discriminant(latency_waitable_object)
                == core::mem::discriminant(&b.latency_waitable_object)
            && dxc_shader_model_opt_eq(&force_shader_model.get(), &b.force_shader_model.get())
            // `Dx12AgilitySDK` is not re-exported by the wgpu crate, so it is compared
            // inline through field access instead of a named helper.
            && match (agility_sdk, &b.agility_sdk) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    a.sdk_version == b.sdk_version
                        && a.sdk_path == b.sdk_path
                        && a.on_load_failure == b.on_load_failure
                }
                _ => false,
            }
    }

    fn dx12_compiler_eq(a: &wgpu_30::Dx12Compiler, b: &wgpu_30::Dx12Compiler) -> bool {
        use wgpu_30::Dx12Compiler as C;
        match (a, b) {
            (C::Fxc, C::Fxc) | (C::StaticDxc, C::StaticDxc) | (C::Auto, C::Auto) => true,
            (C::DynamicDxc { dxc_path: a }, C::DynamicDxc { dxc_path: b }) => a == b,
            _ => false,
        }
    }

    fn dxc_shader_model_opt_eq(
        a: &Option<wgpu_30::DxcShaderModel>,
        b: &Option<wgpu_30::DxcShaderModel>,
    ) -> bool {
        match (a, b) {
            (None, None) => true,
            // Fieldless enum without `PartialEq`; the discriminant is the whole value.
            (Some(a), Some(b)) => core::mem::discriminant(a) == core::mem::discriminant(b),
            _ => false,
        }
    }

    fn noop_backend_options_eq(
        a: &wgpu_30::NoopBackendOptions,
        b: &wgpu_30::NoopBackendOptions,
    ) -> bool {
        let wgpu_30::NoopBackendOptions {
            enable,
            limits,
            features,
            device_type,
            subgroup_min_size,
            subgroup_max_size,
        } = a;
        *enable == b.enable
            && *limits == b.limits
            && *features == b.features
            && *device_type == b.device_type
            && *subgroup_min_size == b.subgroup_min_size
            && *subgroup_max_size == b.subgroup_max_size
    }

    fn memory_budget_thresholds_eq(
        a: &wgpu_30::MemoryBudgetThresholds,
        b: &wgpu_30::MemoryBudgetThresholds,
    ) -> bool {
        let wgpu_30::MemoryBudgetThresholds { for_resource_creation, for_device_loss } = a;
        *for_resource_creation == b.for_resource_creation && *for_device_loss == b.for_device_loss
    }

    fn memory_hints_eq(a: &wgpu_30::MemoryHints, b: &wgpu_30::MemoryHints) -> bool {
        use wgpu_30::MemoryHints as M;
        match (a, b) {
            (M::Performance, M::Performance) | (M::MemoryUsage, M::MemoryUsage) => true,
            (
                M::Manual { suballocated_device_memory_block_size: a },
                M::Manual { suballocated_device_memory_block_size: b },
            ) => a == b,
            _ => false,
        }
    }

    /// This enum describes the different ways to configure WGPU for rendering.
    #[derive(Clone, Debug)]
    #[non_exhaustive]
    #[allow(clippy::large_enum_variant)]
    pub enum WGPUConfiguration {
        /// Use `Manual` if you've initialized WGPU and want to supply the instance, adapter,
        /// device, and queue for use.
        Manual {
            /// The WGPU instance to use.
            instance: wgpu_30::Instance,
            /// The WGPU adapter to use.
            adapter: wgpu_30::Adapter,
            /// The WGPU device to use.
            device: wgpu_30::Device,
            /// The WGPU queue to use.
            queue: wgpu_30::Queue,
        },
        /// Use `Automatic` if you want to let Slint select the WGPU instance, adapter, and
        /// device, but fine-tune aspects such as memory limits or features.
        Automatic(WGPUSettings),
    }

    impl Default for WGPUConfiguration {
        fn default() -> Self {
            Self::Automatic(WGPUSettings::default())
        }
    }

    impl TryFrom<wgpu_30::Texture> for super::super::Image {
        type Error = TextureImportError;

        fn try_from(texture: wgpu_30::Texture) -> Result<Self, Self::Error> {
            if texture.format() != wgpu_30::TextureFormat::Rgba8Unorm
                && texture.format() != wgpu_30::TextureFormat::Rgba8UnormSrgb
            {
                return Err(Self::Error::InvalidFormat);
            }
            let usages = texture.usage();
            if !usages.contains(wgpu_30::TextureUsages::TEXTURE_BINDING)
                || !usages.contains(wgpu_30::TextureUsages::RENDER_ATTACHMENT)
            {
                return Err(Self::Error::InvalidUsage);
            }
            Ok(Self(super::super::ImageInner::WGPUTexture(
                super::super::WGPUTexture::WGPU30Texture(texture),
            )))
        }
    }

    #[derive(Debug, derive_more::Error)]
    #[non_exhaustive]
    /// This enum describes the possible errors that can occur when importing a WGPU texture,
    /// via [`Image::try_from()`](super::super::Image::try_from()).
    pub enum TextureImportError {
        /// The texture format is not supported. The only supported format is Rgba8Unorm and Rgba8UnormSrgb.
        InvalidFormat,
        /// The texture usage must include TEXTURE_BINDING as well as RENDER_ATTACHMENT.
        InvalidUsage,
    }

    impl core::fmt::Display for TextureImportError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
            TextureImportError::InvalidFormat => f.write_str(
                "The texture format is not supported. The only supported format is Rgba8Unorm and Rgba8UnormSrgb",
            ),
            TextureImportError::InvalidUsage => f.write_str(
                "The texture usage must include TEXTURE_BINDING as well as RENDER_ATTACHMENT",
            ),
        }
        }
    }
}

use super::RequestedGraphicsAPI;

/// Internal helper function see if there are any GPU adapters for hardware accelerated rendering.
/// This is used to determine if we should fall back to software rendering (instead of using WGPU
/// software rendering, such as DX12's Warp adapter)
pub fn any_wgpu30_adapters_with_gpu(requested_graphics_api: Option<RequestedGraphicsAPI>) -> bool {
    // On WASM the wgpu init path uses
    // `wgpu::util::new_instance_with_webgpu_detection`, which probes
    // `navigator.gpu.requestAdapter()` asynchronously and falls through
    // to the WebGL backend (compiled in via the wgpu-30 `webgl` feature)
    // when no WebGPU adapter is reachable. So a hardware-accelerated
    // adapter is effectively always available; assume yes here and
    // let the actual init surface a real error if both fail.
    if cfg!(target_family = "wasm") {
        return true;
    }
    let allow_cpu = std::env::var("SLINT_WGPU_CPU").is_ok();
    if allow_cpu {
        return true;
    }
    let (instance, backends) = match requested_graphics_api {
        #[cfg(feature = "unstable-wgpu-30")]
        Some(RequestedGraphicsAPI::WGPU30(api::WGPUConfiguration::Manual { instance, .. })) => {
            (instance, wgpu::Backends::all())
        }
        #[cfg(feature = "unstable-wgpu-30")]
        Some(RequestedGraphicsAPI::WGPU30(api::WGPUConfiguration::Automatic(wgpu30_settings))) => (
            wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu30_settings.backends,
                flags: wgpu30_settings.instance_flags,
                backend_options: wgpu30_settings.backend_options,
                memory_budget_thresholds: wgpu30_settings.instance_memory_budget_thresholds,
                display: None,
            }),
            wgpu30_settings.backends,
        ),
        None => {
            let backends = wgpu::Backends::from_env().unwrap_or_default();

            (
                wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends,
                    flags: wgpu::InstanceFlags::from_build_config().with_env(),
                    backend_options: wgpu::BackendOptions::from_env_or_default(),
                    memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                    display: None,
                }),
                backends,
            )
        }
        Some(_) => return false,
    };
    poll_once(instance.enumerate_adapters(backends))
        .unwrap()
        .into_iter()
        .any(|adapter| adapter.get_info().device_type != wgpu::DeviceType::Cpu)
}

/// Enum to represent different surface targets for WGPU initialization.
pub enum SurfaceTarget {
    /// Standard window handle for windowed rendering. Must also expose a display handle.
    WindowHandle(Box<dyn wgpu::DisplayAndWindowHandle + 'static>),
    /// DRM surface target for direct rendering on Linux KMS.
    Drm(wgpu::SurfaceTargetUnsafe),
}

impl From<Box<dyn wgpu::DisplayAndWindowHandle + 'static>> for SurfaceTarget {
    fn from(handle: Box<dyn wgpu::DisplayAndWindowHandle + 'static>) -> Self {
        Self::WindowHandle(handle)
    }
}

#[cfg(feature = "unstable-wgpu-30")]
fn device_descriptor_from_settings<'a>(
    settings: &'a api::WGPUSettings,
    adapter: &wgpu::Adapter,
) -> wgpu::DeviceDescriptor<'a> {
    wgpu::DeviceDescriptor {
        label: settings.device_label.as_deref(),
        required_features: settings.device_required_features,
        // support images the size of the swapchain
        required_limits: settings.device_required_limits.clone().using_resolution(adapter.limits()),
        experimental_features: settings.device_experimental_features,
        memory_hints: settings.device_memory_hints.clone(),
        trace: wgpu::Trace::default(),
    }
}

fn default_device_descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
    wgpu::DeviceDescriptor {
        label: None,
        required_features: adapter.features() - wgpu::Features::all_experimental_mask(),
        required_limits: adapter.limits(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::default(),
    }
}

/// Internal async helper function to initialize the wgpu instance/adapter/device/queue from either scratch or
/// developer-provided config. This is called by any renderer intending to support WGPU.
pub async fn async_init_instance_adapter_device_queue_surface(
    surface_target: impl Into<SurfaceTarget>,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
    backends_to_avoid: wgpu::Backends,
) -> Result<
    (
        wgpu_30::Instance,
        wgpu_30::Adapter,
        wgpu_30::Device,
        wgpu_30::Queue,
        wgpu_30::Surface<'static>,
    ),
    Box<dyn std::error::Error + Send + Sync + 'static>,
> {
    #![allow(unsafe_code)]

    let surface_target = surface_target.into();

    let create_surface = |instance: &wgpu::Instance| {
        match surface_target {
            SurfaceTarget::WindowHandle(window_handle) => instance.create_surface(window_handle),
            // Safety: The caller ensures the DRM file descriptor in the surface target
            // remains valid for the lifetime of the returned surface, by storing the
            // DrmOutput in the renderer adapter.
            SurfaceTarget::Drm(surface_target_unsafe) => unsafe {
                instance.create_surface_unsafe(surface_target_unsafe)
            },
        }
        .map_err(|e| {
            crate::api::PlatformError::from(alloc::format!(
                "Error creating wgpu window surface: {e}"
            ))
        })
    };

    let (instance, adapter, device, queue, surface) = match requested_graphics_api {
        #[cfg(feature = "unstable-wgpu-30")]
        Some(RequestedGraphicsAPI::WGPU30(api::WGPUConfiguration::Manual {
            instance,
            adapter,
            device,
            queue,
        })) => {
            let surface = create_surface(&instance)?;
            (instance, adapter, device, queue, surface)
        }
        #[cfg(feature = "unstable-wgpu-30")]
        Some(RequestedGraphicsAPI::WGPU30(api::WGPUConfiguration::Automatic(wgpu30_settings))) => {
            let instance =
                wgpu::util::new_instance_with_webgpu_detection(wgpu::InstanceDescriptor {
                    backends: wgpu30_settings.backends & !backends_to_avoid,
                    flags: wgpu30_settings.instance_flags,
                    backend_options: wgpu30_settings.backend_options.clone(),
                    memory_budget_thresholds: wgpu30_settings.instance_memory_budget_thresholds,
                    display: None,
                })
                .await;

            let surface = create_surface(&instance)?;

            let adapter = match wgpu::util::initialize_adapter_from_env(&instance, Some(&surface))
                .await
            {
                Ok(adapter) => Ok(adapter),
                Err(_) => {
                    instance
                        .request_adapter(&wgpu::RequestAdapterOptions {
                            power_preference: wgpu30_settings.power_preference,
                            force_fallback_adapter: false,
                            compatible_surface: Some(&surface),
                            apply_limit_buckets: false,
                        })
                        .await
                }
            }
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync + 'static> {
                alloc::format!("Failed to find an appropriate adapter: {e}").into()
            })?;

            let (device, queue) = adapter
                .request_device(&device_descriptor_from_settings(&wgpu30_settings, &adapter))
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync + 'static> {
                    alloc::format!("Failed to create device: {e}").into()
                })?;

            (instance, adapter, device, queue, surface)
        }
        maybe_native_api @ (None
        | Some(
            RequestedGraphicsAPI::Metal
            | RequestedGraphicsAPI::Vulkan
            | RequestedGraphicsAPI::Direct3D,
        )) => {
            let requested_backends = match &maybe_native_api {
                Some(RequestedGraphicsAPI::Metal) => wgpu::Backends::METAL,
                Some(RequestedGraphicsAPI::Vulkan) => wgpu::Backends::VULKAN,
                Some(RequestedGraphicsAPI::Direct3D) => wgpu::Backends::DX12,
                _ => wgpu::Backends::from_env().unwrap_or_default(),
            };
            let backends = requested_backends & !backends_to_avoid;
            if backends.is_empty() {
                return Err(alloc::format!(
                    "The requested graphics API ({maybe_native_api:?}) is not supported under wgpu on this platform"
                )
                .into());
            }

            let instance =
                wgpu::util::new_instance_with_webgpu_detection(wgpu::InstanceDescriptor {
                    backends,
                    flags: wgpu::InstanceFlags::from_build_config().with_env(),
                    backend_options: wgpu::BackendOptions::from_env_or_default(),
                    memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                    display: None,
                })
                .await;

            let surface = create_surface(&instance)?;

            let adapter =
                wgpu::util::initialize_adapter_from_env_or_default(&instance, Some(&surface))
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync + 'static> {
                        alloc::format!("Failed to find an appropriate adapter: {e}").into()
                    })?;

            let (device, queue) = adapter
                .request_device(&default_device_descriptor(&adapter))
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync + 'static> {
                    alloc::format!("Failed to create device: {e}").into()
                })?;
            (instance, adapter, device, queue, surface)
        }
        Some(other) => {
            return Err(alloc::format!(
                "The requested graphics API ({other:?}) is not supported under wgpu"
            )
            .into());
        }
    };
    Ok((instance, adapter, device, queue, surface))
}

/// Blocking wrapper around [`async_init_instance_adapter_device_queue_surface`] that uses
/// `poll_once` to synchronously drive the future. This works on all platforms except WASM
/// where the wgpu futures don't resolve on first poll.
pub fn init_instance_adapter_device_queue_surface(
    surface_target: impl Into<SurfaceTarget>,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
    backends_to_avoid: wgpu::Backends,
) -> Result<
    (
        wgpu_30::Instance,
        wgpu_30::Adapter,
        wgpu_30::Device,
        wgpu_30::Queue,
        wgpu_30::Surface<'static>,
    ),
    Box<dyn std::error::Error + Send + Sync + 'static>,
> {
    poll_once(async_init_instance_adapter_device_queue_surface(
        surface_target,
        requested_graphics_api,
        backends_to_avoid,
    ))
    .expect("internal error: wgpu setup is not expected to be async")
}

/// Runs [`async_init_instance_adapter_device_queue_surface`] and passes the created
/// objects on to `finalize`. On most platforms the initialization future resolves on
/// the first poll, so this happens synchronously and errors (including `finalize`'s)
/// are returned to the caller. On WASM the initialization does real async work (the
/// WebGPU adapter probe is a JsFuture), so the future is spawned on the event loop
/// via `context`, `finalize` runs when it resolves, and errors can only be logged.
pub fn init_instance_adapter_device_queue_surface_then(
    context: &crate::SlintContext,
    surface_target: impl Into<SurfaceTarget> + 'static,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
    backends_to_avoid: wgpu::Backends,
    finalize: impl FnOnce(
        wgpu_30::Instance,
        wgpu_30::Adapter,
        wgpu_30::Device,
        wgpu_30::Queue,
        wgpu_30::Surface<'static>,
    ) -> Result<(), crate::api::PlatformError>
    + 'static,
) -> Result<(), crate::api::PlatformError> {
    let init_future = async move {
        let (instance, adapter, device, queue, surface) =
            async_init_instance_adapter_device_queue_surface(
                surface_target,
                requested_graphics_api,
                backends_to_avoid,
            )
            .await
            .map_err(|e| {
                crate::api::PlatformError::from(alloc::format!("WGPU initialization failed: {e}"))
            })?;
        finalize(instance, adapter, device, queue, surface)
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = context;
        poll_once(init_future).expect("internal error: wgpu setup is not expected to be async")
    }
    #[cfg(target_arch = "wasm32")]
    {
        context
            .spawn_local(async move {
                if let Err(e) = init_future.await {
                    crate::debug_log!("{e}");
                }
            })
            .map_err(|e| {
                crate::api::PlatformError::from(alloc::format!(
                    "Error spawning async wgpu initialization: {e}"
                ))
            })?;
        Ok(())
    }
}

// Helper function to poll a future once. Remove once the suspension API uses async.
fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    let waker = std::task::Waker::noop();
    let mut ctx = std::task::Context::from_waker(waker);

    let future = std::pin::pin!(future);

    match future.poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}
