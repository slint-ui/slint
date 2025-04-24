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
    ) -> Result<Arc<dyn super::SoftwareBufferDisplay>, PlatformError> {
        let drm_output = DrmOutput::new(device_opener)?;

        //eprintln!("mode {}/{}", width, height);

        let front_buffer: RefCell<DumbBuffer> =
            DumbBuffer::allocate(&drm_output.drm_device, drm_output.size())?.into();
        let back_buffer = DumbBuffer::allocate_with_format(
            &drm_output.drm_device,
            drm_output.size(),
            front_buffer.borrow().format,
            front_buffer.borrow().depth,
            front_buffer.borrow().bpp,
        )?
        .into();
        let in_flight_buffer = DumbBuffer::allocate_with_format(
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

impl DumbBuffer {
    fn allocate(
        device: &impl drm::control::Device,
        (width, height): (u32, u32),
    ) -> Result<Self, PlatformError> {
        let mut last_err = None;
        for (format, depth, bpp) in
            [(drm::buffer::DrmFourcc::Xrgb8888, 24, 32), (drm::buffer::DrmFourcc::Rgb565, 16, 16)]
        {
            match Self::allocate_with_format(device, (width, height), format, depth, bpp) {
                Ok(buf) => return Ok(buf),
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| "Could not allocate drm dumb buffer".into()))
    }

    fn allocate_with_format(
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
