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
    /// Whether [`Self::texture`]'s image has `VK_IMAGE_TILING_LINEAR`, see
    /// [`LinearImport`]. Otherwise a DRM format modifier decides its tiling.
    #[cfg_attr(not(skia_wgpu_30), allow(dead_code))]
    linear_tiling: bool,
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
    buffers: Vec<Buffer>,
    /// Index into `buffers` of the buffer to render the next frame into.
    next: Cell<usize>,
    /// A frame [`Self::present_after`] or [`Self::present_tiled`] hasn't posted yet:
    /// its buffer's index, and the submission that finishes drawing it,
    /// or `None` where the copy from [`Self::tiled`] does.
    #[cfg(skia_wgpu_30)]
    pending: Cell<Option<(usize, Option<wgpu::SubmissionIndex>)>>,
    /// The frame to render into, see [`Self::enable_tiled_frame`].
    #[cfg(skia_wgpu_30)]
    tiled: Option<super::tiled_frame::TiledFrame>,
}

impl GbmDmabufDisplay {
    /// A display with a ring of `buffer_count` scanout buffers.
    /// [`Self::present`] and [`Self::present_after`] each say how many they need.
    pub fn new(
        drm_output: DrmOutput,
        device: &wgpu::Device,
        buffer_count: usize,
    ) -> Result<Self, PlatformError> {
        let gbm_device = gbm::Device::new(drm_output.drm_device.clone())
            .map_err(|e| format!("Error creating gbm device: {e}"))?;

        let (width, height) = drm_output.size();

        // A KMS driver without DRM_CAP_ADDFB2_MODIFIERS, such as i.MX8M Plus's LCDIF,
        // rejects a framebuffer that names a modifier and scans out linear buffers only.
        let display_takes_modifiers = drm::Device::get_driver_capability(
            &drm_output.drm_device,
            drm::DriverCapability::AddFB2Modifiers,
        )
        .is_ok_and(|value| value != 0);

        // Safety: the Vulkan handles are only used for queries and for loading functions,
        // neither of which has side effects, and `properties2` comes from `hal_device`'s instance.
        let (importable_modifiers, linear_import) = unsafe {
            let hal_device = crate::renderer::dmabuf::vulkan_device(device)?;
            let properties2 =
                Properties2::new(hal_device.shared_instance(), hal_device.raw_physical_device())?;
            let importable_modifiers =
                importable_modifiers(&hal_device, &properties2, width, height);
            let by_modifier =
                importable_modifiers.as_ref().is_ok_and(|m| m.contains(&gbm::Modifier::Linear));
            let linear_import =
                LinearImport::new(&hal_device, &properties2, width, height, by_modifier);
            (importable_modifiers, linear_import)
        };

        // The modifiers to allocate from: those Vulkan can render into that the display takes.
        let mut modifiers = match &importable_modifiers {
            Ok(importable_modifiers) if display_takes_modifiers => importable_modifiers.clone(),
            _ => Vec::new(),
        };
        // Only where nothing else works, since a buffer the linear import fails for may have
        // no fallback.
        if modifiers.is_empty() && linear_import.is_possible() {
            modifiers.push(gbm::Modifier::Linear);
        }
        // Why a `LINEAR` buffer gbm allocates from `modifiers` is linear.
        let linear_reason = match &importable_modifiers {
            Err(e) => e.to_string(),
            Ok(_) => "gbm chose them for the display".into(),
        };
        if modifiers.is_empty() {
            let tiling_failure = linear_import.tiling_failure().unwrap_or_default();
            return Err(PlatformError::Other(match importable_modifiers {
                Err(e) if display_takes_modifiers => {
                    format!("{e}, and can't render into a linear one: {tiling_failure}")
                }
                importable_modifiers => {
                    let modifier_reason = match importable_modifiers {
                        Ok(importable_modifiers) => format!(
                            "By DRM format modifier, it imports the scanout format only with \
                             {importable_modifiers:?}"
                        ),
                        Err(e) => e.to_string(),
                    };
                    format!(
                        "The display only scans out linear buffers, which Vulkan can't render \
                         into: {tiling_failure}. {modifier_reason}"
                    )
                }
            }));
        }

        // Notes on slower or riskier paths taken, each printed once.
        let mut notes: Vec<String> = Vec::new();

        let mut buffers = Vec::with_capacity(buffer_count);
        for _ in 0..buffer_count {
            let (buffer, buffer_notes) = Self::allocate_buffer(
                &gbm_device,
                &drm_output,
                device,
                &modifiers,
                display_takes_modifiers,
                &linear_import,
                &linear_reason,
                width,
                height,
            )?;
            buffers.push(buffer);
            for note in buffer_notes {
                if !notes.contains(&note) {
                    notes.push(note);
                }
            }
        }
        for note in notes {
            eprintln!("{note}");
        }
        Ok(Self {
            drm_output,
            buffers,
            next: Cell::new(0),
            #[cfg(skia_wgpu_30)]
            pending: Cell::new(None),
            #[cfg(skia_wgpu_30)]
            tiled: None,
        })
    }

    fn allocate_buffer(
        gbm_device: &gbm::Device<SharedFd>,
        drm_output: &DrmOutput,
        device: &wgpu::Device,
        modifiers: &[gbm::Modifier],
        display_takes_modifiers: bool,
        linear_import: &LinearImport,
        linear_reason: &str,
        width: u32,
        height: u32,
    ) -> Result<(Buffer, Vec<String>), PlatformError> {
        let mut notes = Vec::new();
        // Allocating from the modifiers Vulkan can render into lets gbm pick one that
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
            with_modifiers if linear_import.is_possible() => {
                let reason = match with_modifiers {
                    Ok(_) => "gbm reported no DRM format modifier".to_string(),
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
                     Vulkan can't render into a linear one: {}. By DRM format modifier, it \
                     imports the scanout format only with {modifiers:?}",
                    linear_import.tiling_failure().unwrap_or_default()
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
        if display_takes_modifiers && modifier == gbm::Modifier::Linear {
            let reason = fallback_reason.as_deref().unwrap_or(linear_reason);
            notes.push(format!(
                "Scanning out linear buffers, which can be slower to render into: {reason}"
            ));
        }

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
        .map_err(|e| match &fallback_reason {
            Some(reason) => {
                format!("Error adding the linear gbm buffer as framebuffer, after {reason}: {e}")
            }
            None => format!("Error adding gbm buffer as framebuffer: {e}"),
        })?;
        let framebuffer = OwnedFramebufferHandle { handle, device: drm_output.drm_device.clone() };

        let (texture, linear_tiling, import_note) =
            import_dmabuf_texture(device, &bo, modifier, linear_import)?;
        notes.extend(import_note);

        Ok((Buffer { framebuffer, texture, linear_tiling, _bo: bo }, notes))
    }

    /// The texture to render the next frame into, and its index in the ring.
    pub fn back_buffer(&self) -> (usize, &wgpu::Texture) {
        let index = self.next.get();
        (index, &self.buffers[index].texture)
    }

    /// Posts the back buffer and advances the ring.
    /// The caller must have waited for the GPU work writing it to complete.
    ///
    /// This needs three buffers: one being drawn into, one mid-flip, and one on screen.
    #[cfg_attr(not(feature = "renderer-femtovg-wgpu"), allow(dead_code))]
    pub fn present(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug_assert!(self.buffers.len() >= 3, "see the buffers this needs");
        let index = self.next.get();
        // Waiting here rather than before the frame was drawn overlaps rendering
        // with the flip still in flight. A flip can only be queued once the last
        // one landed.
        self.drm_output.wait_for_page_flip();
        self.drm_output.present_framebuffer(self.buffers[index].framebuffer.handle)?;
        self.next.set((index + 1) % self.buffers.len());
        Ok(())
    }
}

#[cfg(skia_wgpu_30)]
impl GbmDmabufDisplay {
    /// Posts the frame left pending by the last call, if any,
    /// then leaves the back buffer pending until `submission` completed and advances the ring.
    /// A frame that fails to post stays pending, and the back buffer stays the same.
    ///
    /// Posting a frame only once the next one is drawn costs a frame of latency.
    /// In exchange, the GPU finishes the frame while the CPU draws the next one.
    /// Call [`Self::flush`] when no frame follows right away.
    ///
    /// This needs four buffers: one being drawn into, one waiting for the GPU, one mid-flip,
    /// and one on screen.
    pub fn present_after(
        &self,
        device: &wgpu::Device,
        submission: wgpu::SubmissionIndex,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug_assert!(self.buffers.len() >= 4, "see the buffers this needs");
        self.flush(device)?;
        self.pending.set(Some((self.next.get(), Some(submission))));
        self.next.set((self.next.get() + 1) % self.buffers.len());
        Ok(())
    }

    /// Renders into an optimally tiled frame from now on, which [`Self::present_tiled`]
    /// copies into the back buffer, where the scanout buffers are linear. Linear images are
    /// much slower to render into on some GPUs, such as NXP's Vivante ones; Mesa's etnaviv
    /// renders into a tiled shadow as well where it has to. Returns whether it does.
    pub fn enable_tiled_frame(&mut self, device: &wgpu::Device) -> Result<bool, PlatformError> {
        if !self.buffers.iter().all(|buffer| buffer.linear_tiling) {
            return Ok(false);
        }
        self.tiled = Some(super::tiled_frame::TiledFrame::new(
            device,
            self.buffers[0].texture.size(),
            WGPU_FORMAT,
            self.buffers.len(),
        )?);
        Ok(true)
    }

    /// The frame to render into, after [`Self::enable_tiled_frame`].
    pub fn tiled_frame(&self) -> Option<&wgpu::Texture> {
        self.tiled.as_ref().map(|tiled| &tiled.texture)
    }

    /// Copies the tiled frame into the back buffer and posts it like [`Self::present_after`].
    pub fn present_tiled(
        &self,
        device: &wgpu::Device,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug_assert!(self.buffers.len() >= 4, "see the buffers this needs");
        self.flush(device)?;
        let index = self.next.get();
        self.tiled
            .as_ref()
            .expect("a tiled frame")
            .copy_into(index, &self.buffers[index].texture)?;
        self.pending.set(Some((index, None)));
        self.next.set((index + 1) % self.buffers.len());
        Ok(())
    }

    /// Posts the frame [`Self::present_after`] left pending, if any.
    /// A frame that fails to post stays pending.
    pub fn flush(
        &self,
        device: &wgpu::Device,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(pending) = self.pending.take() else { return Ok(()) };
        self.post(device, &pending).inspect_err(|_| self.pending.set(Some(pending)))
    }

    fn post(
        &self,
        device: &wgpu::Device,
        (index, submission): &(usize, Option<wgpu::SubmissionIndex>),
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Why the CPU waits rather than KMS: see "Presentation Paths" in
        // docs/development/window-backend-integration.md.
        match submission {
            Some(submission) => {
                device.poll(wgpu::PollType::Wait {
                    submission_index: Some(submission.clone()),
                    timeout: None,
                })?;
            }
            None => self.tiled.as_ref().expect("a copy from the tiled frame").wait(*index)?,
        }
        // A flip can only be queued once the last one landed. That one also took
        // the frame before last off screen, whose buffer is drawn into next.
        self.drm_output.wait_for_page_flip();
        self.drm_output.present_framebuffer(self.buffers[*index].framebuffer.handle)
    }
}

/// The DRM format modifiers this Vulkan device can import a dma-buf of
/// [`FORMAT`] with, as a scanout buffer of `width` x `height`.
///
/// Handing these to gbm is what makes the two sides agree: gbm knows which
/// modifiers the display can scan out, Vulkan knows which it can import, and only
/// gbm can see both once it is given the list.
///
/// # Safety
/// `properties2` must come from `hal_device`'s instance.
unsafe fn importable_modifiers(
    hal_device: &wgpu::hal::vulkan::Device,
    properties2: &Properties2,
    width: u32,
    height: u32,
) -> Result<Vec<gbm::Modifier>, PlatformError> {
    use ash::vk;

    // Safety: `properties2` comes from `hal_device`'s instance, see `# Safety`, and the
    // queries have no side effects.
    let modifiers = unsafe {
        let physical_device = hal_device.raw_physical_device();

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
            "Vulkan can import no DRM format modifier for a {width}x{height} scanout buffer"
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
/// memory the display scans out. Also returns whether the image has linear tiling.
fn import_dmabuf_texture(
    device: &wgpu::Device,
    bo: &gbm::BufferObject<()>,
    modifier: gbm::Modifier,
    linear_import: &LinearImport,
) -> Result<(wgpu::Texture, bool, Option<String>), PlatformError> {
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

    // Safety: the descriptor describes `bo`, and `texture_from_dmabuf_fd` takes
    // ownership of the fd exported from it.
    let (hal_texture, linear_tiling, note) = unsafe {
        let hal_device = crate::renderer::dmabuf::vulkan_device(device)?;
        let (linear, note) = linear_import.import(&hal_device, bo, modifier, &hal_descriptor)?;
        let (texture, linear_tiling) = match linear {
            Some(texture) => (texture, true),
            None => {
                let fd = bo
                    .fd()
                    .map_err(|e| format!("Error exporting gbm buffer object as dma-buf: {e}"))?;
                let texture = hal_device
                    .texture_from_dmabuf_fd(
                        fd,
                        &hal_descriptor,
                        modifier.into(),
                        bo.stride() as u64,
                        bo.offset(0) as u64,
                    )
                    .map_err(|e| format!("Error importing dma-buf as a Vulkan image: {e}"))?;
                (texture, false)
            }
        };
        (texture, linear_tiling, note)
    };

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
    let texture = unsafe {
        device.create_texture_from_hal::<wgpu::hal::api::Vulkan>(
            hal_texture,
            &descriptor,
            wgpu::TextureUses::UNINITIALIZED,
        )
    };
    Ok((texture, linear_tiling, note))
}

/// Imports linear scanout buffers as `VK_IMAGE_TILING_LINEAR` images.
///
/// `VK_IMAGE_TILING_DRM_FORMAT_MODIFIER_EXT` with `DRM_FORMAT_MOD_LINEAR` describes the
/// same memory, but NXP's Vivante driver ignores that modifier and renders tiled.
///
/// Where that import fails, the modifier import takes over if Vulkan offers `LINEAR`.
struct LinearImport {
    /// Fails with the reason where the driver can't render to a linear image of the display's
    /// size, or can't import one from a dma-buf.
    tiling: Result<ash::khr::external_memory_fd::Device, String>,
    by_modifier: bool,
}

impl LinearImport {
    /// # Safety
    /// `properties2` must come from `hal_device`'s instance.
    unsafe fn new(
        hal_device: &wgpu::hal::vulkan::Device,
        properties2: &Properties2,
        width: u32,
        height: u32,
        by_modifier: bool,
    ) -> Self {
        Self { tiling: unsafe { Self::probe(hal_device, properties2, width, height) }, by_modifier }
    }

    /// Why the `VK_IMAGE_TILING_LINEAR` import doesn't work, where it doesn't.
    fn tiling_failure(&self) -> Option<&str> {
        self.tiling.as_ref().err().map(String::as_str)
    }

    /// Whether Vulkan can render into a linear scanout buffer at all.
    fn is_possible(&self) -> bool {
        self.tiling.is_ok() || self.by_modifier
    }

    /// # Safety
    /// `properties2` must come from `hal_device`'s instance.
    unsafe fn probe(
        hal_device: &wgpu::hal::vulkan::Device,
        properties2: &Properties2,
        width: u32,
        height: u32,
    ) -> Result<ash::khr::external_memory_fd::Device, String> {
        use ash::vk;

        // Safety: `properties2` comes from `hal_device`'s instance, see `# Safety`, and the
        // queries and the function loading have no side effects.
        unsafe {
            let physical_device = hal_device.raw_physical_device();

            let mut properties = vk::FormatProperties2::default();
            properties2.format(physical_device, VK_FORMAT, &mut properties);
            let features = properties.format_properties;
            // Skia takes the image for an optimally tiled one, so linear tiling also has to
            // have `BLIT_SRC` and `BLIT_DST` where optimal tiling has them.
            let needed = NEEDED_TILING_FEATURES
                | (features.optimal_tiling_features
                    & (vk::FormatFeatureFlags::BLIT_SRC | vk::FormatFeatureFlags::BLIT_DST));
            if !features.linear_tiling_features.contains(needed) {
                return Err(format!(
                    "linear tiling lacks {:?}",
                    needed & !features.linear_tiling_features
                ));
            }

            let mut external_info = vk::PhysicalDeviceExternalImageFormatInfo::default()
                .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let info = vk::PhysicalDeviceImageFormatInfo2::default()
                .format(VK_FORMAT)
                .ty(vk::ImageType::TYPE_2D)
                .tiling(vk::ImageTiling::LINEAR)
                .usage(VK_USAGE)
                .push_next(&mut external_info);
            let mut external_properties = vk::ExternalImageFormatProperties::default();
            let mut properties =
                vk::ImageFormatProperties2::default().push_next(&mut external_properties);
            let supported = properties2.image_format(physical_device, &info, &mut properties);
            let extent = properties.image_format_properties.max_extent;
            if let Err(e) = supported {
                return Err(format!("a linear image of the scanout format isn't supported: {e}"));
            }
            if extent.width < width || extent.height < height {
                return Err(format!(
                    "linear images go up to {}x{} only",
                    extent.width, extent.height
                ));
            }
            if !external_properties
                .external_memory_properties
                .external_memory_features
                .contains(vk::ExternalMemoryFeatureFlags::IMPORTABLE)
            {
                return Err("a linear image can't be imported from a dma-buf".into());
            }

            Ok(ash::khr::external_memory_fd::Device::new(
                hal_device.shared_instance().raw_instance(),
                hal_device.raw_device(),
            ))
        }
    }

    /// Imports `bo` as a linear image where its `modifier` is `LINEAR`.
    /// Returns no texture where the modifier import has to take over,
    /// with a note to print where that's because this one failed.
    /// Fails where neither import works.
    ///
    /// # Safety
    /// `hal_descriptor` must describe `bo`.
    unsafe fn import(
        &self,
        hal_device: &wgpu::hal::vulkan::Device,
        bo: &gbm::BufferObject<()>,
        modifier: gbm::Modifier,
        hal_descriptor: &wgpu::hal::TextureDescriptor,
    ) -> Result<(Option<wgpu::hal::vulkan::Texture>, Option<String>), PlatformError> {
        if modifier != gbm::Modifier::Linear {
            return Ok((None, None));
        }
        let imported = match &self.tiling {
            Ok(external_memory_fd) => unsafe {
                Self::import_linear(external_memory_fd, hal_device, bo, hal_descriptor)
            },
            Err(reason) => Err(reason.clone()),
        };
        match imported {
            Ok(texture) => Ok((Some(texture), None)),
            Err(reason) if self.by_modifier => Ok((
                None,
                Some(format!(
                    "Importing linear scanout buffers by their DRM format modifier, which some \
                     drivers render tiled into: {reason}"
                )),
            )),
            Err(reason) => Err(PlatformError::Other(format!(
                "Error importing a linear scanout buffer, which Vulkan can't import by its DRM \
                 format modifier either: {reason}"
            ))),
        }
    }

    /// # Safety
    /// `hal_descriptor` must describe `bo`.
    unsafe fn import_linear(
        external_memory_fd: &ash::khr::external_memory_fd::Device,
        hal_device: &wgpu::hal::vulkan::Device,
        bo: &gbm::BufferObject<()>,
        hal_descriptor: &wgpu::hal::TextureDescriptor,
    ) -> Result<wgpu::hal::vulkan::Texture, String> {
        use ash::vk;
        use std::os::fd::{AsRawFd, IntoRawFd};

        if bo.offset(0) != 0 {
            return Err("the buffer doesn't start at offset 0".into());
        }

        let device = hal_device.raw_device();
        let mut external_memory = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(VK_FORMAT)
            .extent(vk::Extent3D { width: bo.width(), height: bo.height(), depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::LINEAR)
            .usage(VK_USAGE)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut external_memory);
        let image = unsafe { device.create_image(&image_info, None) }
            .map_err(|e| format!("creating the image failed: {e}"))?;
        let destroy_image = || unsafe { device.destroy_image(image, None) };

        let layout = unsafe {
            device.get_image_subresource_layout(
                image,
                vk::ImageSubresource::default().aspect_mask(vk::ImageAspectFlags::COLOR),
            )
        };
        if layout.offset != 0 || layout.row_pitch != u64::from(bo.stride()) {
            destroy_image();
            return Err(format!(
                "Vulkan lays out rows {} bytes apart, gbm {}",
                layout.row_pitch,
                bo.stride()
            ));
        }

        let fd = bo.fd().map_err(|e| {
            destroy_image();
            format!("exporting the buffer as a dma-buf failed: {e}")
        })?;
        let requirements = unsafe { device.get_image_memory_requirements(image) };
        let dmabuf_size = match nix::unistd::lseek(&fd, 0, nix::unistd::Whence::SeekEnd) {
            Ok(size) => size as u64,
            Err(e) => {
                destroy_image();
                return Err(format!("querying the dma-buf's size failed: {e}"));
            }
        };
        if requirements.size > dmabuf_size {
            destroy_image();
            return Err(format!(
                "the image needs {} bytes, the dma-buf holds {dmabuf_size}",
                requirements.size
            ));
        }
        let mut fd_properties = vk::MemoryFdPropertiesKHR::default();
        if let Err(e) = unsafe {
            external_memory_fd.get_memory_fd_properties(
                vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
                fd.as_raw_fd(),
                &mut fd_properties,
            )
        } {
            destroy_image();
            return Err(format!("querying the buffer's memory types failed: {e}"));
        }
        let memory_types = requirements.memory_type_bits & fd_properties.memory_type_bits;
        if memory_types == 0 {
            destroy_image();
            return Err("no memory type fits both the image and the buffer".into());
        }

        let mut import_info = vk::ImportMemoryFdInfoKHR::default()
            .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
            .fd(fd.as_raw_fd());
        let mut dedicated_info = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_types.trailing_zeros())
            .push_next(&mut import_info)
            .push_next(&mut dedicated_info);
        let memory = match unsafe { device.allocate_memory(&allocate_info, None) } {
            Ok(memory) => memory,
            Err(e) => {
                destroy_image();
                return Err(format!("importing the buffer's memory failed: {e}"));
            }
        };
        // A successful import takes ownership of the fd.
        let _ = fd.into_raw_fd();

        if let Err(e) = unsafe { device.bind_image_memory(image, memory, 0) } {
            unsafe { device.free_memory(memory, None) };
            destroy_image();
            return Err(format!("binding the buffer's memory failed: {e}"));
        }

        // Safety: `image` was created from this device with `hal_descriptor`'s size,
        // format and usage, and wgpu-hal destroys it and frees `memory` with the texture.
        Ok(unsafe {
            hal_device.texture_from_raw(
                image,
                hal_descriptor,
                None,
                wgpu::hal::vulkan::TextureMemory::Dedicated(memory),
            )
        })
    }
}
