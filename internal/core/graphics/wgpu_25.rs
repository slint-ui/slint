// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]

/*!
This module contains types that are public and re-exported in the slint-rs as well as the slint-interpreter crate as public API,
in particular the `BackendSelector` type, to configure the WGPU-based renderer(s).
*/

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
                    fence_behavior: Default::default(),
                },
                noop: wgpu_25::NoopBackendOptions::default(),
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
