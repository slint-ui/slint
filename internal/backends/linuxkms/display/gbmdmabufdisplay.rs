// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Presentation for wgpu renderers on drivers without `VK_EXT_acquire_drm_display`.
//!
//! Vulkan never sees the display here.
//! GBM allocates a ring of scanout buffers, and wgpu imports each as a texture via dma-buf.
//! The frame reaches the screen through the DRM page flip the OpenGL and software paths use.
//!
//! See the presentation paths in `docs/development/window-backend-integration.md`
//! for how this fits the rest of the linuxkms backend.

// cSpell: ignore ADDFB dmabuf etnaviv LCDIF SCANOUT subresource

use std::cell::Cell;

use drm::control::Device;
use i_slint_core::platform::PlatformError;
use wgpu_30 as wgpu;

use crate::drmoutput::{DrmOutput, OwnedFramebufferHandle, SharedFd};

/// Number of buffers in the ring.
/// A frame is drawn while the previous one is still on its way to the screen,
/// so at any moment one buffer is being rendered into, one is mid-flip and one is on screen.
pub const BUFFER_COUNT: usize = 3;

/// The scanout format.
/// `Xrgb8888` is the one format every DRM plane is required to support,
/// and its byte order matches wgpu's `Bgra8Unorm`.
const FORMAT: gbm::Format = gbm::Format::Xrgb8888;
const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
/// The same format again, for the modifier query, which goes through Vulkan directly.
/// Keep the three in step.
const VK_FORMAT: ash::vk::Format = ash::vk::Format::B8G8R8A8_UNORM;

struct Buffer {
    framebuffer: OwnedFramebufferHandle,
    texture: wgpu::Texture,
    /// Dropped after the framebuffer and the texture that refer to it.
    _bo: gbm::BufferObject<()>,
}

/// A gbm buffer as a framebuffer without a modifier, for a display that takes none.
struct ImplicitLayout<'a>(&'a gbm::BufferObject<()>);

impl drm::buffer::PlanarBuffer for ImplicitLayout<'_> {
    fn size(&self) -> (u32, u32) {
        drm::buffer::PlanarBuffer::size(self.0)
    }
    fn format(&self) -> drm::buffer::DrmFourcc {
        drm::buffer::PlanarBuffer::format(self.0)
    }
    fn modifier(&self) -> Option<drm::buffer::DrmModifier> {
        None
    }
    fn pitches(&self) -> [u32; 4] {
        drm::buffer::PlanarBuffer::pitches(self.0)
    }
    fn handles(&self) -> [Option<drm::buffer::Handle>; 4] {
        drm::buffer::PlanarBuffer::handles(self.0)
    }
    fn offsets(&self) -> [u32; 4] {
        drm::buffer::PlanarBuffer::offsets(self.0)
    }
}

pub struct GbmDmabufDisplay {
    pub drm_output: DrmOutput,
    buffers: [Buffer; BUFFER_COUNT],
    /// Index into `buffers` of the buffer to render the next frame into.
    next: Cell<usize>,
}

impl GbmDmabufDisplay {
    pub fn new(drm_output: DrmOutput, device: &wgpu::Device) -> Result<Self, PlatformError> {
        let gbm_device = gbm::Device::new(drm_output.drm_device.clone())
            .map_err(|e| format!("Error creating gbm device: {e}"))?;

        let (width, height) = drm_output.size();

        let importable_modifiers = importable_modifiers(device, width, height)?;

        // A KMS driver without DRM_CAP_ADDFB2_MODIFIERS, such as i.MX8M Plus's LCDIF,
        // rejects a framebuffer that names a modifier and scans out linear buffers only.
        let display_takes_modifiers = drm::Device::get_driver_capability(
            &drm_output.drm_device,
            drm::DriverCapability::AddFB2Modifiers,
        )
        .is_ok_and(|value| value != 0);
        let modifiers = if display_takes_modifiers {
            importable_modifiers
        } else if importable_modifiers.contains(&gbm::Modifier::Linear) {
            vec![gbm::Modifier::Linear]
        } else {
            return Err(PlatformError::Other(format!(
                "The display only scans out linear buffers, but Vulkan can import the scanout \
                 format only with {importable_modifiers:?}"
            )));
        };

        let mut buffers = Vec::with_capacity(BUFFER_COUNT);
        let mut linear_fallback = None;
        for _ in 0..BUFFER_COUNT {
            let (buffer, fallback) = Self::allocate_buffer(
                &gbm_device,
                &drm_output,
                device,
                &modifiers,
                display_takes_modifiers,
                width,
                height,
            )?;
            buffers.push(buffer);
            linear_fallback = linear_fallback.or(fallback);
        }
        if let Some(reason) = linear_fallback {
            eprintln!("Scanning out linear buffers, which can be slower to render into: {reason}");
        }
        let buffers: [Buffer; BUFFER_COUNT] = buffers
            .try_into()
            .unwrap_or_else(|_| unreachable!("the loop pushes exactly BUFFER_COUNT buffers"));

        Ok(Self { drm_output, buffers, next: Cell::new(0) })
    }

    fn allocate_buffer(
        gbm_device: &gbm::Device<SharedFd>,
        drm_output: &DrmOutput,
        device: &wgpu::Device,
        modifiers: &[gbm::Modifier],
        display_takes_modifiers: bool,
        width: u32,
        height: u32,
    ) -> Result<(Buffer, Option<String>), PlatformError> {
        // Allocating from the modifiers Vulkan can import lets gbm pick one that
        // is also scanout-capable, which is what `SCANOUT` asks of it.
        let with_modifiers = gbm_device.create_buffer_object_with_modifiers2::<()>(
            width,
            height,
            FORMAT,
            modifiers.iter().copied(),
            gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING,
        );

        // A gbm backend without modifier support fails that, or ignores the list and
        // answers `Invalid`: a layout of the driver's choosing, which Vulkan can't name.
        // Asking for `LINEAR` instead gives a layout Vulkan can name.
        let (bo, fallback_reason) = match with_modifiers {
            Ok(bo) if bo.modifier() != gbm::Modifier::Invalid => (bo, None),
            with_modifiers if modifiers.contains(&gbm::Modifier::Linear) => {
                let reason = match with_modifiers {
                    Ok(_) => "gbm reports no DRM format modifier".to_string(),
                    Err(e) => format!("allocating with a DRM format modifier failed: {e}"),
                };
                let bo = gbm_device
                    .create_buffer_object::<()>(
                        width,
                        height,
                        FORMAT,
                        gbm::BufferObjectFlags::SCANOUT
                            | gbm::BufferObjectFlags::RENDERING
                            | gbm::BufferObjectFlags::LINEAR,
                    )
                    .map_err(|e| {
                        format!(
                            "Error allocating a linear gbm buffer object for scanout, after \
                             {reason}: {e}"
                        )
                    })?;
                (bo, Some(reason))
            }
            Ok(_) => {
                return Err(PlatformError::Other(format!(
                    "The gbm driver reports no DRM format modifier for the scanout buffer, and \
                     Vulkan can't import a linear one, only {modifiers:?}"
                )));
            }
            Err(e) => {
                return Err(PlatformError::Other(format!(
                    "Error allocating a gbm buffer object for scanout among {} DRM format \
                     modifier(s): {e}",
                    modifiers.len()
                )));
            }
        };
        let reported = bo.modifier();
        let modifier =
            if reported == gbm::Modifier::Invalid { gbm::Modifier::Linear } else { reported };
        // A display that scans out linear buffers only gets nothing slower.
        let linear_fallback = (display_takes_modifiers && modifier == gbm::Modifier::Linear)
            .then(|| fallback_reason.unwrap_or_else(|| "gbm chose them for the display".into()));

        // Only claim a modifier to KMS when gbm named one and the display takes
        // modifiers. A buffer whose layout is implicit has none to pass, even where
        // `LINEAR` made it linear for the Vulkan import above.
        let handle = if display_takes_modifiers && reported != gbm::Modifier::Invalid {
            drm_output.drm_device.add_planar_framebuffer(&bo, drm::control::FbCmd2Flags::MODIFIERS)
        } else {
            drm_output
                .drm_device
                .add_planar_framebuffer(&ImplicitLayout(&bo), drm::control::FbCmd2Flags::empty())
        }
        .map_err(|e| format!("Error adding gbm buffer as framebuffer: {e}"))?;
        let framebuffer = OwnedFramebufferHandle { handle, device: drm_output.drm_device.clone() };

        let texture = import_dmabuf_texture(device, &bo, modifier)?;

        Ok((Buffer { framebuffer, texture, _bo: bo }, linear_fallback))
    }

    /// The texture to render the next frame into, and its index in the ring.
    pub fn back_buffer(&self) -> (usize, &wgpu::Texture) {
        let index = self.next.get();
        (index, &self.buffers[index].texture)
    }

    /// Posts the back buffer and advances the ring.
    /// The caller must have waited for the GPU work writing it to complete.
    pub fn present(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let index = self.next.get();
        // Waiting here rather than before the frame was drawn overlaps rendering
        // with the flip still in flight, see `BUFFER_COUNT`. A flip can only be
        // queued once the last one landed.
        self.drm_output.wait_for_page_flip();
        self.drm_output.present_framebuffer(self.buffers[index].framebuffer.handle)?;
        self.next.set((index + 1) % BUFFER_COUNT);
        Ok(())
    }
}

/// The DRM format modifiers this Vulkan device can import a dma-buf of
/// [`FORMAT`] with, as a scanout buffer of `width` x `height`.
///
/// Handing these to gbm is what makes the two sides agree: gbm knows which
/// modifiers the display can scan out, Vulkan knows which it can import, and only
/// gbm can see both once it is given the list.
fn importable_modifiers(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> Result<Vec<gbm::Modifier>, PlatformError> {
    use ash::vk;

    // Safety: only the physical device and instance handles are read, and the
    // queries have no side effects.
    let modifiers = unsafe {
        let hal_device = crate::renderer::dmabuf::vulkan_device(device)?;
        let physical_device = hal_device.raw_physical_device();
        let properties2 = Properties2::new(hal_device.shared_instance(), physical_device)?;

        // The first call reports how many entries there are, the second fills them in.
        let mut list = vk::DrmFormatModifierPropertiesListEXT::default();
        let mut properties = vk::FormatProperties2::default().push_next(&mut list);
        properties2.format(physical_device, VK_FORMAT, &mut properties);

        let mut entries = vec![
            vk::DrmFormatModifierPropertiesEXT::default();
            list.drm_format_modifier_count as usize
        ];
        list.p_drm_format_modifier_properties = entries.as_mut_ptr();
        let mut properties = vk::FormatProperties2::default().push_next(&mut list);
        properties2.format(physical_device, VK_FORMAT, &mut properties);

        let importable = |modifier: u64| {
            let mut modifier_info = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
                .drm_format_modifier(modifier)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            let mut external_info = vk::PhysicalDeviceExternalImageFormatInfo::default()
                .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let info = vk::PhysicalDeviceImageFormatInfo2::default()
                .format(VK_FORMAT)
                .ty(vk::ImageType::TYPE_2D)
                .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                .usage(VK_USAGE)
                .push_next(&mut modifier_info)
                .push_next(&mut external_info);
            let mut external_properties = vk::ExternalImageFormatProperties::default();
            let mut properties =
                vk::ImageFormatProperties2::default().push_next(&mut external_properties);
            let supported = properties2.image_format(physical_device, &info, &mut properties);
            let extent = properties.image_format_properties.max_extent;
            supported.is_ok()
                && extent.width >= width
                && extent.height >= height
                && external_properties
                    .external_memory_properties
                    .external_memory_features
                    .contains(vk::ExternalMemoryFeatureFlags::IMPORTABLE)
        };

        entries
            .into_iter()
            // The import passes the first plane only, so layouts with more, such as
            // compressed ones with a metadata plane, are out.
            .filter(|entry| {
                entry.drm_format_modifier_plane_count == 1
                    && entry.drm_format_modifier_tiling_features.contains(NEEDED_TILING_FEATURES)
                    && importable(entry.drm_format_modifier)
            })
            .map(|entry| gbm::Modifier::from(entry.drm_format_modifier))
            .collect::<Vec<_>>()
    };

    if modifiers.is_empty() {
        return Err(PlatformError::Other(format!(
            "The Vulkan driver can import no DRM format modifier for a {width}x{height} scanout \
             buffer"
        )));
    }

    Ok(modifiers)
}

/// The format features a scanout buffer's tiling needs.
///
/// Skia blends into its render targets and copies from and into them.
/// It copies without scaling through `vkCmdCopyImage`, which needs the transfer features only,
/// and uses a blit only to scale.
const NEEDED_TILING_FEATURES: ash::vk::FormatFeatureFlags = ash::vk::FormatFeatureFlags::from_raw(
    ash::vk::FormatFeatureFlags::COLOR_ATTACHMENT.as_raw()
        | ash::vk::FormatFeatureFlags::COLOR_ATTACHMENT_BLEND.as_raw()
        | ash::vk::FormatFeatureFlags::TRANSFER_SRC.as_raw()
        | ash::vk::FormatFeatureFlags::TRANSFER_DST.as_raw(),
);

/// The Vulkan image usages of a scanout buffer, see [`import_dmabuf_texture`].
const VK_USAGE: ash::vk::ImageUsageFlags = ash::vk::ImageUsageFlags::from_raw(
    ash::vk::ImageUsageFlags::COLOR_ATTACHMENT.as_raw()
        | ash::vk::ImageUsageFlags::TRANSFER_SRC.as_raw()
        | ash::vk::ImageUsageFlags::TRANSFER_DST.as_raw(),
);

/// The `vkGetPhysicalDevice*Properties2` queries, from `VK_KHR_get_physical_device_properties2`
/// where wgpu enabled it, as wgpu's own queries do, and otherwise from Vulkan 1.1.
enum Properties2 {
    Core(ash::Instance),
    Extension(ash::khr::get_physical_device_properties2::Instance),
}

impl Properties2 {
    /// # Safety
    /// `physical_device` must belong to `instance`.
    unsafe fn new(
        instance: &wgpu::hal::vulkan::InstanceShared,
        physical_device: ash::vk::PhysicalDevice,
    ) -> Result<Self, PlatformError> {
        use ash::khr::get_physical_device_properties2 as extension;
        // wgpu enables the extension wherever the driver offers it, whatever the version.
        if instance.extensions().contains(&extension::NAME) {
            return Ok(Self::Extension(extension::Instance::new(
                instance.entry(),
                instance.raw_instance(),
            )));
        }
        // Physical device queries follow the version of both the instance and the device.
        let device_api_version = unsafe {
            instance.raw_instance().get_physical_device_properties(physical_device).api_version
        };
        if instance.instance_api_version().min(device_api_version) >= ash::vk::API_VERSION_1_1 {
            Ok(Self::Core(instance.raw_instance().clone()))
        } else {
            Err(PlatformError::Other(
                "The Vulkan driver offers neither Vulkan 1.1 nor \
                 VK_KHR_get_physical_device_properties2, which choosing a scanout buffer's \
                 layout needs"
                    .into(),
            ))
        }
    }

    unsafe fn format(
        &self,
        physical_device: ash::vk::PhysicalDevice,
        format: ash::vk::Format,
        properties: &mut ash::vk::FormatProperties2<'_>,
    ) {
        unsafe {
            match self {
                Self::Core(instance) => instance.get_physical_device_format_properties2(
                    physical_device,
                    format,
                    properties,
                ),
                Self::Extension(instance) => instance.get_physical_device_format_properties2(
                    physical_device,
                    format,
                    properties,
                ),
            }
        }
    }

    unsafe fn image_format(
        &self,
        physical_device: ash::vk::PhysicalDevice,
        info: &ash::vk::PhysicalDeviceImageFormatInfo2<'_>,
        properties: &mut ash::vk::ImageFormatProperties2<'_>,
    ) -> ash::prelude::VkResult<()> {
        unsafe {
            match self {
                Self::Core(instance) => instance.get_physical_device_image_format_properties2(
                    physical_device,
                    info,
                    properties,
                ),
                Self::Extension(instance) => instance.get_physical_device_image_format_properties2(
                    physical_device,
                    info,
                    properties,
                ),
            }
        }
    }
}

/// Imports `bo`'s dma-buf as a wgpu texture that renders directly into the
/// memory the display scans out.
fn import_dmabuf_texture(
    device: &wgpu::Device,
    bo: &gbm::BufferObject<()>,
    modifier: gbm::Modifier,
) -> Result<wgpu::Texture, PlatformError> {
    let fd = bo.fd().map_err(|e| format!("Error exporting gbm buffer object as dma-buf: {e}"))?;

    let size = wgpu::Extent3d { width: bo.width(), height: bo.height(), depth_or_array_layers: 1 };

    let hal_descriptor = wgpu::hal::TextureDescriptor {
        label: Some("Slint linuxkms scanout buffer"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WGPU_FORMAT,
        // Skia only wraps render targets that allow copying from and into them.
        // Matches `VK_USAGE`.
        usage: wgpu::TextureUses::COLOR_TARGET
            | wgpu::TextureUses::COPY_SRC
            | wgpu::TextureUses::COPY_DST,
        memory_flags: wgpu::hal::MemoryFlags::empty(),
        view_formats: vec![],
    };

    // Safety: the descriptor describes the buffer object `fd` was exported from,
    // and `texture_from_dmabuf_fd` takes ownership of the fd.
    let hal_texture = unsafe {
        crate::renderer::dmabuf::vulkan_device(device)?.texture_from_dmabuf_fd(
            fd,
            &hal_descriptor,
            modifier.into(),
            bo.stride() as u64,
            bo.offset(0) as u64,
        )
    }
    .map_err(|e| format!("Error importing dma-buf as a Vulkan image: {e}"))?;

    let descriptor = wgpu::TextureDescriptor {
        label: hal_descriptor.label,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WGPU_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    };

    // Safety: `hal_texture` was just created from this device and matches the
    // descriptor. Its contents are undefined until the first frame is drawn.
    Ok(unsafe {
        device.create_texture_from_hal::<wgpu::hal::api::Vulkan>(
            hal_texture,
            &descriptor,
            wgpu::TextureUses::UNINITIALIZED,
        )
    })
}
