// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use drm::control::Device;
use gbm::AsRaw;
use i_slint_core::platform::PlatformError;

use crate::drmoutput::{DrmOutput, SharedFd};

struct OwnedFramebufferHandle {
    handle: drm::control::framebuffer::Handle,
    device: SharedFd,
}

impl Drop for OwnedFramebufferHandle {
    fn drop(&mut self) {
        self.device.destroy_framebuffer(self.handle).ok();
    }
}

pub struct GbmDisplay {
    pub drm_output: DrmOutput,
    gbm_surface: gbm::Surface<OwnedFramebufferHandle>,
    gbm_device: gbm::Device<SharedFd>,
}

impl GbmDisplay {
    pub fn new(drm_output: DrmOutput) -> Result<GbmDisplay, PlatformError> {
        //eprintln!("mode {}/{}", width, height);

        let gbm_device = gbm::Device::new(drm_output.drm_device.clone())
            .map_err(|e| format!("Error creating gbm device: {e}"))?;

        let (width, height) = drm_output.size();
        let gbm_surface = gbm_device
            .create_surface::<OwnedFramebufferHandle>(
                width,
                height,
                gbm::Format::Xrgb8888,
                gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING,
            )
            .map_err(|e| format!("Error creating gbm surface: {e}"))?;

        Ok(GbmDisplay { drm_output, gbm_surface, gbm_device })
    }
}

impl super::Presenter for GbmDisplay {
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
        let mut front_buffer = unsafe {
            self.gbm_surface
                .lock_front_buffer()
                .map_err(|e| format!("Error locking gmb surface front buffer: {e}"))?
        };

        // TODO: support modifiers
        // TODO: consider falling back to the old non-planar API
        let fb = self
            .drm_output
            .drm_device
            .add_planar_framebuffer(&front_buffer, &[None, None, None, None], 0)
            .map_err(|e| format!("Error adding gbm buffer as framebuffer: {e}"))?;

        front_buffer
            .set_userdata(OwnedFramebufferHandle {
                handle: fb,
                device: self.drm_output.drm_device.clone(),
            })
            .map_err(|e| format!("Error setting userdata on gbm surface front buffer: {e}"))?;

        self.drm_output.present(front_buffer, fb, ready_for_next_animation_frame)
    }

    fn is_ready_to_present(&self) -> bool {
        self.drm_output.is_ready_to_present()
    }
}

impl raw_window_handle::HasWindowHandle for GbmDisplay {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let mut gbm_surface_handle = raw_window_handle::GbmWindowHandle::empty();
        gbm_surface_handle.gbm_surface = self.gbm_surface.as_raw() as _;

        // Safety: This is safe because the handle remains valid; the next rwh release provides `new()` without unsafe.
        let active_handle = unsafe { raw_window_handle::ActiveHandle::new_unchecked() };

        Ok(unsafe {
            raw_window_handle::WindowHandle::borrow_raw(
                raw_window_handle::RawWindowHandle::Gbm(gbm_surface_handle),
                active_handle,
            )
        })
    }
}

impl raw_window_handle::HasDisplayHandle for GbmDisplay {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        let mut gbm_display_handle = raw_window_handle::GbmDisplayHandle::empty();
        gbm_display_handle.gbm_device = self.gbm_device.as_raw() as _;

        Ok(unsafe {
            raw_window_handle::DisplayHandle::borrow_raw(raw_window_handle::RawDisplayHandle::Gbm(
                gbm_display_handle,
            ))
        })
    }
}
