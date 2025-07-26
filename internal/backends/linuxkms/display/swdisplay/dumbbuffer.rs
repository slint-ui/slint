// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::sync::Arc;

use crate::drmoutput::DrmOutput;
use drm::control::Device;
use i_slint_core::platform::PlatformError;

pub struct DumbBufferDisplay {
    drm_output: DrmOutput,
    /// Currently displayed buffer
    front_buffer: RefCell<DumbBuffer>,
    /// Buffer next to be rendered into
    back_buffer: RefCell<DumbBuffer>,
    /// Buffer currently on the way to the display, to become front_buffer
    in_flight_buffer: RefCell<DumbBuffer>,
}

impl DumbBufferDisplay {
    pub fn new(
        device_opener: &crate::DeviceOpener,
        renderer_formats: &[drm::buffer::DrmFourcc],
    ) -> Result<Arc<dyn super::SoftwareBufferDisplay>, PlatformError> {
        let drm_output = DrmOutput::new(device_opener)?;

        let available_formats = drm_output.get_supported_formats()?;

        let format = super::negotiate_format(renderer_formats, &available_formats)
            .ok_or_else(|| PlatformError::Other(
                format!("No compatible format found for DumbBuffer. Renderer supports: {:?}, FB supports: {:?}",
                        renderer_formats, available_formats).into()))?;

        let (depth, bpp) = pixel_format_params(format)
            .ok_or_else(|| format!("Cannot get depth and bpp for pixel format: {format:?}"))?;

        let front_buffer: RefCell<DumbBuffer> =
            DumbBuffer::allocate(&drm_output.drm_device, drm_output.size(), format, depth, bpp)
                .map_err(|err| format!("Could not allocate drm dumb buffer: {err}"))?
                .into();

        let back_buffer = DumbBuffer::allocate(
            &drm_output.drm_device,
            drm_output.size(),
            front_buffer.borrow().format,
            front_buffer.borrow().depth,
            front_buffer.borrow().bpp,
        )?
        .into();
        let in_flight_buffer = DumbBuffer::allocate(
            &drm_output.drm_device,
            drm_output.size(),
            front_buffer.borrow().format,
            front_buffer.borrow().depth,
            front_buffer.borrow().bpp,
        )?
        .into();

        Ok(Arc::new(Self { drm_output, front_buffer, back_buffer, in_flight_buffer }))
    }
}

impl super::SoftwareBufferDisplay for DumbBufferDisplay {
    fn size(&self) -> (u32, u32) {
        self.drm_output.size()
    }

    fn map_back_buffer(
        &self,
        callback: &mut dyn FnMut(
            &'_ mut [u8],
            u8,
            drm::buffer::DrmFourcc,
        ) -> Result<(), PlatformError>,
    ) -> Result<(), PlatformError> {
        let mut back_buffer = self.back_buffer.borrow_mut();
        let age = back_buffer.age;
        let format = back_buffer.format;
        self.drm_output
            .drm_device
            .map_dumb_buffer(&mut back_buffer.buffer_handle)
            .map_err(|e| PlatformError::Other(format!("Error mapping dumb buffer: {e}").into()))
            .and_then(|mut buffer| callback(buffer.as_mut(), age, format))
    }

    fn as_presenter(self: Arc<Self>) -> Arc<dyn crate::display::Presenter> {
        self
    }
}

impl crate::display::Presenter for DumbBufferDisplay {
    fn present(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.drm_output.wait_for_page_flip();

        self.back_buffer.swap(&self.front_buffer);
        self.front_buffer.swap(&self.in_flight_buffer);

        self.in_flight_buffer.borrow_mut().age = 1;
        for buffer in [&self.back_buffer, &self.front_buffer] {
            let mut buffer_borrow = buffer.borrow_mut();
            if buffer_borrow.age != 0 {
                buffer_borrow.age += 1;
            }
        }

        // TODO: dirty framebuffer
        self.drm_output.present(
            self.in_flight_buffer.borrow().buffer_handle,
            self.in_flight_buffer.borrow().fb_handle,
        )?;
        Ok(())
    }
}

struct DumbBuffer {
    fb_handle: drm::control::framebuffer::Handle,
    buffer_handle: drm::control::dumbbuffer::DumbBuffer,
    age: u8,
    format: drm::buffer::DrmFourcc,
    depth: u32,
    bpp: u32,
}

/// Returns the pixel depth and bits-per-pixel values for a given DRM pixel format.
fn pixel_format_params(format: drm::buffer::DrmFourcc) -> Option<(u32, u32)> {
    match format {
        // 32-bit RGB formats
        drm::buffer::DrmFourcc::Xrgb8888 => Some((24, 32)),
        drm::buffer::DrmFourcc::Argb8888 => Some((32, 32)),
        drm::buffer::DrmFourcc::Xbgr8888 => Some((24, 32)),
        drm::buffer::DrmFourcc::Abgr8888 => Some((32, 32)),
        drm::buffer::DrmFourcc::Rgbx8888 => Some((24, 32)),
        drm::buffer::DrmFourcc::Rgba8888 => Some((32, 32)),
        drm::buffer::DrmFourcc::Bgrx8888 => Some((24, 32)),
        drm::buffer::DrmFourcc::Bgra8888 => Some((32, 32)),

        // 30-bit RGB formats (10 bits per channel)
        drm::buffer::DrmFourcc::Xrgb2101010 => Some((30, 32)),
        drm::buffer::DrmFourcc::Argb2101010 => Some((32, 32)),
        drm::buffer::DrmFourcc::Xbgr2101010 => Some((30, 32)),
        drm::buffer::DrmFourcc::Abgr2101010 => Some((32, 32)),
        drm::buffer::DrmFourcc::Rgbx1010102 => Some((30, 32)),
        drm::buffer::DrmFourcc::Rgba1010102 => Some((32, 32)),
        drm::buffer::DrmFourcc::Bgrx1010102 => Some((30, 32)),
        drm::buffer::DrmFourcc::Bgra1010102 => Some((32, 32)),

        // 24-bit RGB formats
        drm::buffer::DrmFourcc::Rgb888 => Some((24, 24)),
        drm::buffer::DrmFourcc::Bgr888 => Some((24, 24)),

        // 16-bit RGB formats
        drm::buffer::DrmFourcc::Rgb565 => Some((16, 16)),
        drm::buffer::DrmFourcc::Bgr565 => Some((16, 16)),
        drm::buffer::DrmFourcc::Xrgb1555 => Some((15, 16)),
        drm::buffer::DrmFourcc::Argb1555 => Some((16, 16)),
        drm::buffer::DrmFourcc::Xbgr1555 => Some((15, 16)),
        drm::buffer::DrmFourcc::Abgr1555 => Some((16, 16)),
        drm::buffer::DrmFourcc::Rgbx5551 => Some((15, 16)),
        drm::buffer::DrmFourcc::Rgba5551 => Some((16, 16)),
        drm::buffer::DrmFourcc::Bgrx5551 => Some((15, 16)),
        drm::buffer::DrmFourcc::Bgra5551 => Some((16, 16)),
        drm::buffer::DrmFourcc::Xrgb4444 => Some((12, 16)),
        drm::buffer::DrmFourcc::Argb4444 => Some((16, 16)),
        drm::buffer::DrmFourcc::Xbgr4444 => Some((12, 16)),
        drm::buffer::DrmFourcc::Abgr4444 => Some((16, 16)),
        drm::buffer::DrmFourcc::Rgbx4444 => Some((12, 16)),
        drm::buffer::DrmFourcc::Rgba4444 => Some((16, 16)),
        drm::buffer::DrmFourcc::Bgrx4444 => Some((12, 16)),
        drm::buffer::DrmFourcc::Bgra4444 => Some((16, 16)),

        // 8-bit indexed formats
        drm::buffer::DrmFourcc::C8 => Some((8, 8)),

        // YUV packed formats
        drm::buffer::DrmFourcc::Yuyv => Some((16, 16)),
        drm::buffer::DrmFourcc::Yvyu => Some((16, 16)),
        drm::buffer::DrmFourcc::Uyvy => Some((16, 16)),
        drm::buffer::DrmFourcc::Vyuy => Some((16, 16)),

        // YUV planar formats
        drm::buffer::DrmFourcc::Yuv420 => Some((12, 12)), // 4:2:0 = 8 + 2 + 2 = 12 bpp
        drm::buffer::DrmFourcc::Yvu420 => Some((12, 12)),
        drm::buffer::DrmFourcc::Yuv422 => Some((16, 16)), // 4:2:2 = 8 + 4 + 4 = 16 bpp
        drm::buffer::DrmFourcc::Yvu422 => Some((16, 16)),
        drm::buffer::DrmFourcc::Yuv444 => Some((24, 24)), // 4:4:4 = 8 + 8 + 8 = 24 bpp
        drm::buffer::DrmFourcc::Yvu444 => Some((24, 24)),

        // NV formats (semi-planar YUV)
        drm::buffer::DrmFourcc::Nv12 => Some((12, 12)),
        drm::buffer::DrmFourcc::Nv21 => Some((12, 12)),
        drm::buffer::DrmFourcc::Nv16 => Some((16, 16)),
        drm::buffer::DrmFourcc::Nv61 => Some((16, 16)),
        drm::buffer::DrmFourcc::Nv24 => Some((24, 24)),
        drm::buffer::DrmFourcc::Nv42 => Some((24, 24)),

        _ => None,
    }
}

impl DumbBuffer {
    fn allocate(
        device: &impl drm::control::Device,
        (width, height): (u32, u32),
        format: drm::buffer::DrmFourcc,
        depth: u32,
        bpp: u32,
    ) -> Result<Self, PlatformError> {
        let buffer_handle =
            device.create_dumb_buffer((width, height), format, bpp).map_err(|e| {
                format!(
                    "Error creating dumb buffer ({}/{}): {} for format {format}",
                    width, height, e
                )
            })?;
        let fb_handle = device.add_framebuffer(&buffer_handle, depth, bpp).map_err(|e| {
            format!("Error creating framebuffer for dumb buffer for format {format}: {e}")
        })?;

        Ok(Self { fb_handle, buffer_handle, age: 0, format, depth, bpp })
    }
}
