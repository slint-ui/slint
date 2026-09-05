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

struct Buffer {
    framebuffer: OwnedFramebufferHandle,
    texture: wgpu::Texture,
    /// Dropped after the framebuffer and the texture that refer to it.
    _bo: gbm::BufferObject<()>,
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

        let mut buffers = Vec::with_capacity(BUFFER_COUNT);
        for _ in 0..BUFFER_COUNT {
            buffers.push(Self::allocate_buffer(&gbm_device, &drm_output, device, width, height)?);
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

        let framebuffer = OwnedFramebufferHandle {
            handle: drm_output
                .drm_device
                .add_planar_framebuffer(&bo, drm::control::FbCmd2Flags::MODIFIERS)
                .map_err(|e| format!("Error adding gbm buffer as framebuffer: {e}"))?,
            device: drm_output.drm_device.clone(),
        };

        let texture = import_dmabuf_texture(device, &bo, modifier)?;

        Ok(Buffer { framebuffer, texture, _bo: bo })
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
        usage: wgpu::TextureUses::COLOR_TARGET
            | wgpu::TextureUses::COPY_SRC
            | wgpu::TextureUses::COPY_DST,
        memory_flags: wgpu::hal::MemoryFlags::empty(),
        view_formats: vec![],
    };

    // Safety: the descriptor describes the buffer object `fd` was exported from,
    // and `texture_from_dmabuf_fd` takes ownership of the fd.
    let hal_texture = unsafe {
        crate::renderer::skia_dmabuf::vulkan_device(device)?.texture_from_dmabuf_fd(
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
