// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]

/*!
This module contains types that are public and re-exported in the slint-rs as well as the slint-interpreter crate as public API,
in particular the `BackendSelector` type, to configure the WGPU-based renderer(s).
*/

pub use wgpu_24 as wgpu;

/// This data structure provides settings for initializing WGPU renderers.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct WGPUSettings {
    /// The backends to use for the WGPU instance.
    pub backends: wgpu_24::Backends,
    /// The different options that are given to the selected backends.
    pub backend_options: wgpu_24::BackendOptions,
    /// The flags to fine-tune behaviour of the WGPU instance.
    pub instance_flags: wgpu_24::InstanceFlags,

    /// The power preference is used to influence the WGPU adapter selection.
    pub power_preference: wgpu_24::PowerPreference,

    /// The label for the device. This is used to identify the device in debugging tools.
    pub device_label: Option<std::borrow::Cow<'static, str>>,
    /// The required features for the device.
    pub device_required_features: wgpu_24::Features,
    /// The required limits for the device.
    pub device_required_limits: wgpu_24::Limits,
    /// The memory hints for the device.
    pub device_memory_hints: wgpu_24::MemoryHints,
}

impl Default for WGPUSettings {
    fn default() -> Self {
        let backends = wgpu_24::Backends::from_env().unwrap_or_default();
        let dx12_shader_compiler = wgpu_24::Dx12Compiler::from_env().unwrap_or_default();
        let gles_minor_version = wgpu_24::Gles3MinorVersion::from_env().unwrap_or_default();

        Self {
            backends,
            backend_options: wgpu_24::BackendOptions {
                dx12: wgpu_24::Dx12BackendOptions { shader_compiler: dx12_shader_compiler },
                gl: wgpu_24::GlBackendOptions { gles_minor_version },
            },
            instance_flags: wgpu_24::InstanceFlags::from_build_config().with_env(),

            power_preference: wgpu_24::PowerPreference::from_env().unwrap_or_default(),

            device_label: None,
            device_required_features: wgpu_24::Features::empty(),
            device_required_limits: wgpu_24::Limits::downlevel_webgl2_defaults(),
            device_memory_hints: wgpu_24::MemoryHints::MemoryUsage,
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
        instance: wgpu_24::Instance,
        /// The WGPU adapter to use.
        adapter: wgpu_24::Adapter,
        /// The WGPU device to use.
        device: wgpu_24::Device,
        /// The WGPU queue to use.
        queue: wgpu_24::Queue,
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

impl TryFrom<wgpu_24::Texture> for super::Image {
    type Error = crate::graphics::wgpu_24::TextureImportError;

    fn try_from(texture: wgpu_24::Texture) -> Result<Self, Self::Error> {
        if texture.format() != wgpu_24::TextureFormat::Rgba8Unorm
            && texture.format() != wgpu_24::TextureFormat::Rgba8UnormSrgb
        {
            return Err(Self::Error::InvalidFormat);
        }
        let usages = texture.usage();
        if !usages.contains(wgpu_24::TextureUsages::TEXTURE_BINDING)
            || !usages.contains(wgpu_24::TextureUsages::RENDER_ATTACHMENT)
        {
            return Err(Self::Error::InvalidUsage);
        }
        Ok(Self(super::ImageInner::WGPUTexture(super::WGPUTexture::WGPU24Texture(texture))))
    }
}

#[derive(Debug, derive_more::Error)]
#[non_exhaustive]
/// This enum describes the possible errors that can occur when importing a WGPU texture,
/// via [`Image::try_from()`](super::Image::try_from()).
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
