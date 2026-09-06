// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Presentation for wgpu renderers on drivers without `VK_EXT_acquire_drm_display`.
//!
//! Vulkan never sees the display here. GBM allocates a ring of scanout-capable
//! buffers, each is exported as a dma-buf and imported into wgpu as a texture,
//! and the finished frame reaches the screen through the same DRM page flip the
//! OpenGL and software paths use.
//!
//! See the presentation paths in `docs/development/window-backend-integration.md`
//! for how this fits the rest of the linuxkms backend.

// cSpell: ignore SCANOUT dmabuf

use std::cell::Cell;

use drm::control::Device;
use i_slint_core::platform::PlatformError;
use wgpu_30 as wgpu;

use crate::drmoutput::{DrmOutput, SharedFd};

/// Number of buffers in the ring. A frame is drawn while the previous one is
/// still on its way to the screen, so at any moment one buffer is being rendered
/// into, one is mid-flip and one is on screen.
const BUFFER_COUNT: usize = 3;

/// The scanout format. `Xrgb8888` is the one format every DRM plane is required
/// to support, and its byte order matches wgpu's `Bgra8Unorm`.
const FORMAT: gbm::Format = gbm::Format::Xrgb8888;
const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
/// The same format again, for the modifier query, which goes through Vulkan
/// directly. Keep the three in step.
const VK_FORMAT: ash::vk::Format = ash::vk::Format::B8G8R8A8_UNORM;

struct Buffer {
    /// Kept alive for as long as the framebuffer and the texture refer to it.
    _bo: gbm::BufferObject<()>,
    modifier: gbm::Modifier,
    /// Whether [`Self::modifier`] was assumed rather than reported by gbm.
    modifier_assumed: bool,
    framebuffer: drm::control::framebuffer::Handle,
    texture: wgpu::Texture,
}

pub struct GbmDmabufDisplay {
    pub drm_output: DrmOutput,
    buffers: [Buffer; BUFFER_COUNT],
    /// Index into `buffers` of the buffer to render the next frame into.
    next: Cell<usize>,
}

impl Drop for GbmDmabufDisplay {
    fn drop(&mut self) {
        for buffer in &self.buffers {
            self.drm_output.drm_device.destroy_framebuffer(buffer.framebuffer).ok();
        }
    }
}

impl GbmDmabufDisplay {
    pub fn new(drm_output: DrmOutput, device: &wgpu::Device) -> Result<Self, PlatformError> {
        if !device.features().contains(wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF) {
            return Err(PlatformError::Other(
                "The Vulkan driver supports neither VK_EXT_acquire_drm_display for direct \
                 display access nor VK_EXT_external_memory_dma_buf and \
                 VK_EXT_image_drm_format_modifier for rendering into a scanout buffer"
                    .into(),
            ));
        }

        let gbm_device = gbm::Device::new(drm_output.drm_device.clone())
            .map_err(|e| format!("Error creating gbm device: {e}"))?;

        let (width, height) = drm_output.size();

        let importable_modifiers = importable_modifiers(device)?;

        let mut buffers = Vec::with_capacity(BUFFER_COUNT);
        for _ in 0..BUFFER_COUNT {
            buffers.push(Self::allocate_buffer(
                &gbm_device,
                &drm_output,
                device,
                &importable_modifiers,
                width,
                height,
            )?);
        }
        let buffers: [Buffer; BUFFER_COUNT] = buffers
            .try_into()
            .unwrap_or_else(|_| unreachable!("the loop pushes exactly BUFFER_COUNT buffers"));

        eprintln!(
            "Scanning out {BUFFER_COUNT} gbm buffers with DRM format modifier {:?}{}",
            buffers[0].modifier,
            if buffers[0].modifier_assumed { " (assumed: gbm reports none)" } else { "" }
        );

        Ok(Self { drm_output, buffers, next: Cell::new(0) })
    }

    fn allocate_buffer(
        gbm_device: &gbm::Device<SharedFd>,
        drm_output: &DrmOutput,
        device: &wgpu::Device,
        importable_modifiers: &[gbm::Modifier],
        width: u32,
        height: u32,
    ) -> Result<Buffer, PlatformError> {
        // Allocating from the modifiers Vulkan can import lets gbm pick one that
        // is also scanout-capable, which is what `SCANOUT` asks of it. Allocating
        // without a list instead leaves the modifier up to the driver, and one
        // that answers `Invalid` describes no layout Vulkan could import.
        let bo = gbm_device
            .create_buffer_object_with_modifiers2::<()>(
                width,
                height,
                FORMAT,
                importable_modifiers.iter().copied(),
                gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING,
            )
            .map_err(|e| {
                format!(
                    "Error allocating a gbm buffer object for scanout among the {} modifier(s) \
                     Vulkan can import: {e}",
                    importable_modifiers.len()
                )
            })?;

        // A gbm backend with no modifier support ignores the list it was handed
        // and answers `Invalid`, meaning the layout is whatever the driver
        // implicitly uses. For a backend in that position — the dumb buffer paths,
        // software rendering — that layout is linear, so say so rather than give
        // up: Vulkan has no way to import a layout that isn't named.
        let reported = bo.modifier();
        let modifier = match reported {
            gbm::Modifier::Invalid
                if bo.plane_count() == 1
                    && importable_modifiers.contains(&gbm::Modifier::Linear) =>
            {
                gbm::Modifier::Linear
            }
            gbm::Modifier::Invalid => {
                return Err(PlatformError::Other(format!(
                    "The gbm driver reports no DRM format modifier for the scanout buffer, and \
                     none can be assumed: it has {} plane(s) and Vulkan can import \
                     {importable_modifiers:?}",
                    bo.plane_count()
                )));
            }
            modifier => modifier,
        };

        // Only claim a modifier to KMS when gbm named one. A buffer whose layout
        // is implicit has none to pass, even where `modifier` assumed linear for
        // the Vulkan import above. Same rule as `GbmDisplay::present`.
        let flags = if reported == gbm::Modifier::Invalid {
            drm::control::FbCmd2Flags::empty()
        } else {
            drm::control::FbCmd2Flags::MODIFIERS
        };

        let framebuffer = drm_output
            .drm_device
            .add_planar_framebuffer(&bo, flags)
            .map_err(|e| format!("Error adding gbm buffer as framebuffer: {e}"))?;

        let texture = import_dmabuf_texture(device, &bo, modifier).inspect_err(|_| {
            drm_output.drm_device.destroy_framebuffer(framebuffer).ok();
        })?;

        Ok(Buffer {
            _bo: bo,
            modifier,
            modifier_assumed: reported == gbm::Modifier::Invalid,
            framebuffer,
            texture,
        })
    }

    /// The texture to render the next frame into.
    pub fn back_buffer(&self) -> &wgpu::Texture {
        &self.buffers[self.next.get()].texture
    }

    /// Posts the back buffer and advances the ring. The caller must have waited
    /// for the GPU work writing it to complete.
    pub fn present(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let index = self.next.get();
        // Waiting here rather than before the frame was drawn is what overlaps
        // rendering with the flip still in flight, and what makes the third
        // buffer necessary. A flip can only be queued once the last one landed.
        self.drm_output.wait_for_page_flip();
        // The buffers outlive the flip, so no keep-alive is handed over.
        self.drm_output.present_framebuffer(self.buffers[index].framebuffer)?;
        self.next.set((index + 1) % BUFFER_COUNT);
        Ok(())
    }
}

/// The DRM format modifiers this Vulkan device can import [`FORMAT`] with, as a
/// color target.
///
/// Handing these to gbm is what makes the two sides agree: gbm knows which
/// modifiers the display can scan out, Vulkan knows which it can import, and only
/// gbm can see both once it is given the list.
fn importable_modifiers(device: &wgpu::Device) -> Result<Vec<gbm::Modifier>, PlatformError> {
    // Safety: only the physical device and instance handles are read, and the
    // query has no side effects.
    let modifiers = unsafe {
        let hal_device = device
            .as_hal::<wgpu::hal::api::Vulkan>()
            .ok_or_else(|| PlatformError::from("The wgpu device is not a Vulkan device"))?;
        let instance = hal_device.shared_instance().raw_instance();
        let physical_device = hal_device.raw_physical_device();

        // The first call reports how many entries there are, the second fills them in.
        let mut list = ash::vk::DrmFormatModifierPropertiesListEXT::default();
        let mut properties = ash::vk::FormatProperties2::default().push_next(&mut list);
        instance.get_physical_device_format_properties2(
            physical_device,
            VK_FORMAT,
            &mut properties,
        );

        let mut entries = vec![
            ash::vk::DrmFormatModifierPropertiesEXT::default();
            list.drm_format_modifier_count as usize
        ];
        list.p_drm_format_modifier_properties = entries.as_mut_ptr();
        let mut properties = ash::vk::FormatProperties2::default().push_next(&mut list);
        instance.get_physical_device_format_properties2(
            physical_device,
            VK_FORMAT,
            &mut properties,
        );

        entries
            .into_iter()
            .filter(|entry| {
                entry
                    .drm_format_modifier_tiling_features
                    .contains(ash::vk::FormatFeatureFlags::COLOR_ATTACHMENT)
            })
            .map(|entry| gbm::Modifier::from(entry.drm_format_modifier))
            .collect::<Vec<_>>()
    };

    if modifiers.is_empty() {
        return Err(PlatformError::Other(
            "The Vulkan driver can import no DRM format modifier for the scanout format as a \
             color target"
                .into(),
        ));
    }

    Ok(modifiers)
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
        usage: wgpu::TextureUses::COLOR_TARGET,
        memory_flags: wgpu::hal::MemoryFlags::empty(),
        view_formats: vec![],
    };

    // Safety: the descriptor describes the buffer object `fd` was exported from,
    // and `texture_from_dmabuf_fd` takes ownership of the fd.
    let hal_texture = unsafe {
        let hal_device = device
            .as_hal::<wgpu::hal::api::Vulkan>()
            .ok_or_else(|| PlatformError::from("The wgpu device is not a Vulkan device"))?;
        hal_device.texture_from_dmabuf_fd(
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
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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
