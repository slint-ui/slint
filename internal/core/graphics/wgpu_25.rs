// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]

/*!
This module contains types that are public and re-exported in the slint-rs as well as the slint-interpreter crate as public API,
in particular the `BackendSelector` type, to configure the WGPU-based renderer(s).
*/

use alloc::boxed::Box;

pub use wgpu_25 as wgpu;

pub mod api {
    /*!
    This module contains types that are public and re-exported in the slint-rs as well as the slint-interpreter crate as public API.
    */

    pub use super::wgpu;

    /// This data structure provides settings for initializing WGPU renderers.
    #[derive(Clone, Debug)]
    #[non_exhaustive]
    pub struct WGPUSettings {
        /// The backends to use for the WGPU instance.
        pub backends: wgpu_25::Backends,
        /// The different options that are given to the selected backends.
        pub backend_options: wgpu_25::BackendOptions,
        /// The flags to fine-tune behaviour of the WGPU instance.
        pub instance_flags: wgpu_25::InstanceFlags,

        /// The power preference is used to influence the WGPU adapter selection.
        pub power_preference: wgpu_25::PowerPreference,

        /// The label for the device. This is used to identify the device in debugging tools.
        pub device_label: Option<std::borrow::Cow<'static, str>>,
        /// The required features for the device.
        pub device_required_features: wgpu_25::Features,
        /// The required limits for the device.
        pub device_required_limits: wgpu_25::Limits,
        /// The memory hints for the device.
        pub device_memory_hints: wgpu_25::MemoryHints,
    }

    impl Default for WGPUSettings {
        fn default() -> Self {
            let backends = wgpu_25::Backends::from_env().unwrap_or_default();
            let dx12_shader_compiler = wgpu_25::Dx12Compiler::from_env().unwrap_or_default();
            let gles_minor_version = wgpu_25::Gles3MinorVersion::from_env().unwrap_or_default();

            Self {
                backends,
                backend_options: wgpu_25::BackendOptions {
                    dx12: wgpu_25::Dx12BackendOptions { shader_compiler: dx12_shader_compiler },
                    gl: wgpu_25::GlBackendOptions {
                        gles_minor_version,
                        fence_behavior: wgpu_25::GlFenceBehavior::default(),
                    },
                    noop: wgpu::NoopBackendOptions::default(),
                },
                instance_flags: wgpu_25::InstanceFlags::from_build_config().with_env(),

                power_preference: wgpu_25::PowerPreference::from_env().unwrap_or_default(),

                device_label: None,
                device_required_features: wgpu_25::Features::empty(),
                device_required_limits: wgpu_25::Limits::downlevel_webgl2_defaults(),
                device_memory_hints: wgpu_25::MemoryHints::MemoryUsage,
            }
        }
    }

    /// This enum describes the different ways to configure WGPU for rendering.
    #[derive(Clone, Debug)]
    #[non_exhaustive]
    pub enum WGPUConfiguration {
        /// Use `Manual` if you've initialized WGPU and want to supply the instance, adapter,
        /// device, and queue for use.
        Manual {
            /// The WGPU instance to use.
            instance: wgpu_25::Instance,
            /// The WGPU adapter to use.
            adapter: wgpu_25::Adapter,
            /// The WGPU device to use.
            device: wgpu_25::Device,
            /// The WGPU queue to use.
            queue: wgpu_25::Queue,
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

    impl TryFrom<wgpu_25::Texture> for super::super::Image {
        type Error = TextureImportError;

        fn try_from(texture: wgpu_25::Texture) -> Result<Self, Self::Error> {
            if texture.format() != wgpu_25::TextureFormat::Rgba8Unorm
                && texture.format() != wgpu_25::TextureFormat::Rgba8UnormSrgb
            {
                return Err(Self::Error::InvalidFormat);
            }
            let usages = texture.usage();
            if !usages.contains(wgpu_25::TextureUsages::TEXTURE_BINDING)
                || !usages.contains(wgpu_25::TextureUsages::RENDER_ATTACHMENT)
            {
                return Err(Self::Error::InvalidUsage);
            }
            Ok(Self(super::super::ImageInner::WGPUTexture(
                super::super::WGPUTexture::WGPU25Texture(texture),
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

/// Internal help function to initialize the wgpu instance/adapter/device/queue from either scratch or
/// developer-provided config. This is called by any renderer intending to support WGPU.
pub fn init_instance_adapter_device_queue_surface(
    window_handle: Box<dyn wgpu::WindowHandle + 'static>,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
) -> Result<
    (
        wgpu_25::Instance,
        wgpu_25::Adapter,
        wgpu_25::Device,
        wgpu_25::Queue,
        wgpu_25::Surface<'static>,
    ),
    Box<dyn std::error::Error + Send + Sync + 'static>,
> {
    let (instance, adapter, device, queue, surface) = match requested_graphics_api {
        Some(RequestedGraphicsAPI::WGPU25(api::WGPUConfiguration::Manual {
            instance,
            adapter,
            device,
            queue,
        })) => {
            let surface = instance.create_surface(window_handle).unwrap();
            (instance, adapter, device, queue, surface)
        }
        Some(RequestedGraphicsAPI::WGPU25(api::WGPUConfiguration::Automatic(wgpu25_settings))) => {
            // wgpu uses async here, but the returned future is ready on first poll on all platforms except WASM,
            // which we don't support right now.
            let instance = poll_once(async {
                wgpu::util::new_instance_with_webgpu_detection(&wgpu::InstanceDescriptor {
                    backends: wgpu25_settings.backends,
                    flags: wgpu25_settings.instance_flags,
                    backend_options: wgpu25_settings.backend_options,
                })
                .await
            })
            .expect("internal error: wgpu instance creation is not expected to be async");

            let surface = instance.create_surface(window_handle).unwrap();

            // wgpu uses async here, but the returned future is ready on first poll on all platforms except WASM,
            // which we don't support right now.
            let adapter = poll_once(async {
                match wgpu::util::initialize_adapter_from_env(&instance, Some(&surface)) {
                    Ok(adapter) => Ok(adapter),
                    Err(_) => {
                        instance
                            .request_adapter(&wgpu::RequestAdapterOptions {
                                power_preference: wgpu25_settings.power_preference,
                                force_fallback_adapter: false,
                                compatible_surface: Some(&surface),
                            })
                            .await
                    }
                }
                .expect("Failed to find an appropriate adapter")
            })
            .expect("internal error: wgpu adapter creation is not expected to be async");

            let (device, queue) = poll_once(async {
                adapter
                    .request_device(&wgpu::DeviceDescriptor {
                        label: wgpu25_settings.device_label.as_deref(),
                        required_features: wgpu25_settings.device_required_features,
                        // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                        required_limits: wgpu25_settings
                            .device_required_limits
                            .using_resolution(adapter.limits()),
                        memory_hints: wgpu25_settings.device_memory_hints,
                        trace: wgpu::Trace::default(),
                    })
                    .await
                    .expect("Failed to create device")
            })
            .expect("internal error: wgpu device creation is not expected to be async");

            (instance, adapter, device, queue, surface)
        }
        None => {
            let backends = wgpu::Backends::from_env().unwrap_or_default();
            let dx12_shader_compiler = wgpu::Dx12Compiler::from_env().unwrap_or_default();
            let gles_minor_version = wgpu::Gles3MinorVersion::from_env().unwrap_or_default();

            // wgpu uses async here, but the returned future is ready on first poll on all platforms except WASM,
            // which we don't support right now.
            let instance = poll_once(async {
                wgpu::util::new_instance_with_webgpu_detection(&wgpu::InstanceDescriptor {
                    backends,
                    flags: wgpu::InstanceFlags::from_build_config().with_env(),
                    backend_options: wgpu::BackendOptions {
                        dx12: wgpu::Dx12BackendOptions { shader_compiler: dx12_shader_compiler },
                        gl: wgpu::GlBackendOptions {
                            gles_minor_version,
                            fence_behavior: wgpu::GlFenceBehavior::default(),
                        },
                        noop: wgpu::NoopBackendOptions::default(),
                    },
                })
                .await
            })
            .expect("internal error: wgpu instance creation is not expected to be async");

            let surface = instance.create_surface(window_handle).unwrap();

            // wgpu uses async here, but the returned future is ready on first poll on all platforms except WASM,
            // which we don't support right now.
            let adapter = poll_once(async {
                wgpu::util::initialize_adapter_from_env_or_default(&instance, Some(&surface))
                    .await
                    .expect("Failed to find an appropriate adapter")
            })
            .expect("internal error: wgpu adapter creation is not expected to be async");

            let (device, queue) = poll_once(async {
                adapter
                    .request_device(&wgpu::DeviceDescriptor {
                        label: None,
                        required_features: wgpu::Features::empty(),
                        // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                        required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                            .using_resolution(adapter.limits()),
                        memory_hints: wgpu::MemoryHints::MemoryUsage,
                        trace: wgpu::Trace::default(),
                    })
                    .await
                    .expect("Failed to create device")
            })
            .expect("internal error: wgpu device creation is not expected to be async");
            (instance, adapter, device, queue, surface)
        }
        Some(_) => {
            return Err(
                "The FemtoVG WGPU renderer does not implement renderer selection by graphics API"
                    .into(),
            );
        }
    };
    Ok((instance, adapter, device, queue, surface))
}

// Helper function to poll a future once. Remove once the suspension API uses async.
fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    struct DummyWaker();
    impl std::task::Wake for DummyWaker {
        fn wake(self: std::sync::Arc<Self>) {}
    }

    let waker = std::sync::Arc::new(DummyWaker()).into();
    let mut ctx = std::task::Context::from_waker(&waker);

    let future = std::pin::pin!(future);

    match future.poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}
