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

/// Number of buffers in the ring. Two would leave the GPU waiting on the buffer
/// currently being scanned out; the third gives it something to render into
/// while a flip is pending.
const BUFFER_COUNT: usize = 3;

/// The scanout format. `Xrgb8888` is the one format every DRM plane is required
/// to support, and its byte order matches wgpu's `Bgra8Unorm`.
const FORMAT: gbm::Format = gbm::Format::Xrgb8888;
const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

struct Buffer {
    /// Kept alive for as long as the framebuffer and the texture refer to it.
    _bo: gbm::BufferObject<()>,
    framebuffer: drm::control::framebuffer::Handle,
    texture: wgpu::Texture,
}

pub struct GbmDmabufDisplay {
    pub drm_output: DrmOutput,
    buffers: Vec<Buffer>,
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

        let buffers = (0..BUFFER_COUNT)
            .map(|_| Self::allocate_buffer(&gbm_device, &drm_output, device, width, height))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { drm_output, buffers, next: Cell::new(0) })
    }

    fn allocate_buffer(
        gbm_device: &gbm::Device<SharedFd>,
        drm_output: &DrmOutput,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Result<Buffer, PlatformError> {
        let bo = gbm_device
            .create_buffer_object::<()>(
                width,
                height,
                FORMAT,
                gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING,
            )
            .map_err(|e| format!("Error allocating gbm buffer object for scanout: {e}"))?;

        // Vulkan needs an explicit modifier to describe the image layout, so a
        // driver that won't name the one it picked can't be imported. Proper
        // negotiation would intersect the plane's IN_FORMATS with the modifiers
        // Vulkan reports for the format; neither drm-rs nor wgpu exposes those
        // lists yet.
        let modifier = bo.modifier();
        if modifier == gbm::Modifier::Invalid {
            return Err(PlatformError::Other(
                "The gbm driver reports no DRM format modifier for the scanout buffer, which \
                 Vulkan needs in order to import it"
                    .into(),
            ));
        }

        let framebuffer = drm_output
            .drm_device
            .add_planar_framebuffer(&bo, drm::control::FbCmd2Flags::MODIFIERS)
            .map_err(|e| format!("Error adding gbm buffer as framebuffer: {e}"))?;

        let texture = import_dmabuf_texture(device, &bo, modifier).inspect_err(|_| {
            drm_output.drm_device.destroy_framebuffer(framebuffer).ok();
        })?;

        Ok(Buffer { _bo: bo, framebuffer, texture })
    }

    /// The texture to render the next frame into.
    pub fn back_buffer(&self) -> &wgpu::Texture {
        &self.buffers[self.next.get()].texture
    }

    /// Posts the back buffer and advances the ring. The caller must have waited
    /// for the GPU work writing it to complete.
    pub fn present(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let index = self.next.get();
        // The buffers outlive the flip, so no keep-alive is handed over.
        self.drm_output.present_framebuffer(self.buffers[index].framebuffer)?;
        self.next.set((index + 1) % self.buffers.len());
        Ok(())
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
