// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use std::cell::RefCell;

use crate::drmoutput::DrmOutput;
use drm::control::Device;
use i_slint_core::platform::PlatformError;

pub struct SoftwareBufferDisplay {
    pub drm_output: DrmOutput,
    front_buffer: RefCell<DumbBuffer>,
    back_buffer: RefCell<DumbBuffer>,
}

impl SoftwareBufferDisplay {
    pub fn new(drm_output: DrmOutput) -> Result<Self, PlatformError> {
        //eprintln!("mode {}/{}", width, height);

        let front_buffer = DumbBuffer::allocate(&drm_output.drm_device, drm_output.size())?.into();
        let back_buffer = DumbBuffer::allocate(&drm_output.drm_device, drm_output.size())?.into();

        Ok(Self { drm_output, front_buffer, back_buffer })
    }

    pub fn map_back_buffer(
        &self,
        callback: &mut dyn FnMut(
            drm::control::dumbbuffer::DumbMapping<'_>,
        ) -> Result<(), PlatformError>,
    ) -> Result<(), PlatformError> {
        let mut back_buffer = self.back_buffer.borrow_mut();
        self.drm_output
            .drm_device
            .map_dumb_buffer(&mut back_buffer.buffer_handle)
            .map_err(|e| PlatformError::Other(format!("Error mapping dumb buffer: {e}").into()))
            .and_then(callback)
    }
}

impl super::Presenter for SoftwareBufferDisplay {
    fn register_page_flip_handler(
        &self,
        event_loop_handle: crate::calloop_backend::EventLoopHandle,
    ) -> Result<(), PlatformError> {
        self.drm_output.register_page_flip_handler(event_loop_handle)
    }

    fn present_with_next_frame_callback(
        &self,
        ready_for_next_animation_frame: Box<dyn FnOnce()>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: dirty framebuffer
        self.front_buffer.swap(&self.back_buffer);
        self.drm_output.present(
            self.front_buffer.borrow().buffer_handle,
            self.front_buffer.borrow().fb_handle,
            ready_for_next_animation_frame,
        )?;
        Ok(())
    }

    fn is_ready_to_present(&self) -> bool {
        self.drm_output.is_ready_to_present()
    }
}

struct DumbBuffer {
    fb_handle: drm::control::framebuffer::Handle,
    buffer_handle: drm::control::dumbbuffer::DumbBuffer,
}

impl DumbBuffer {
    fn allocate(
        device: &impl drm::control::Device,
        (width, height): (u32, u32),
    ) -> Result<Self, PlatformError> {
        let buffer_handle = device
            .create_dumb_buffer((width, height), drm::buffer::DrmFourcc::Xrgb8888, 32)
            .map_err(|e| format!("Error creating dumb buffer ({}/{}): {}", width, height, e))?;
        let fb_handle = device
            .add_framebuffer(&buffer_handle, 24, 32)
            .map_err(|e| format!("Error creating framebuffer for dumb buffer: {e}"))?;

        Ok(Self { fb_handle, buffer_handle })
    }
}
