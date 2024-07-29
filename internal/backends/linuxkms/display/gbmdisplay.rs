// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
    surface_format: drm::buffer::DrmFourcc,
}

impl GbmDisplay {
    pub fn new(drm_output: DrmOutput) -> Result<GbmDisplay, PlatformError> {
        //eprintln!("mode {}/{}", width, height);

        let gbm_device = gbm::Device::new(drm_output.drm_device.clone())
            .map_err(|e| format!("Error creating gbm device: {e}"))?;

        let surface_format = gbm::Format::Xrgb8888;

        let (width, height) = drm_output.size();
        let gbm_surface = gbm_device
            .create_surface::<OwnedFramebufferHandle>(
                width,
                height,
                surface_format,
                gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING,
            )
            .map_err(|e| format!("Error creating gbm surface: {e}"))?;

        Ok(GbmDisplay { drm_output, gbm_surface, gbm_device, surface_format })
    }

    pub fn config_template_builder(&self) -> glutin::config::ConfigTemplateBuilder {
        let mut config_template_builder = glutin::config::ConfigTemplateBuilder::new();

        // Some drivers (like mali) report BAD_MATCH when trying to create a window surface for an xrgb backed
        // gbm surface with an EGL config that has an alpha size of 8. Disable alpha explicitly to accomodate.
        if matches!(self.surface_format, drm::buffer::DrmFourcc::Xrgb8888) {
            config_template_builder =
                config_template_builder.with_transparency(false).with_alpha_size(0);
        }

        config_template_builder
    }

    pub fn filter_gl_config(&self, config: &glutin::config::Config) -> bool {
        match &config {
            glutin::config::Config::Egl(egl_config) => {
                drm::buffer::DrmFourcc::try_from(egl_config.native_visual())
                    .map_or(false, |egl_config_fourcc| egl_config_fourcc == self.surface_format)
            }
            _ => false,
        }
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
        // Workaround for https://github.com/Smithay/gbm.rs/issues/36:
        // call gbm_sys::gbm_surface_lock_front_buffer() directly to
        // avoid the failing has_free_buffers() check on the vivante gbm backend.

        let mut front_buffer = unsafe {
            let surface = self.gbm_surface.as_raw() as _;
            let bo = gbm_sys::gbm_surface_lock_front_buffer(surface);
            if bo.is_null() {
                return Err(format!("Could not lock gbm front buffer").into());
            }
            gbm_rs_workaround::GbmBo { bo, surface }
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
        Ok(unsafe {
            let gbm_surface_handle = raw_window_handle::GbmWindowHandle::new(
                std::ptr::NonNull::from(&*self.gbm_surface.as_raw()).cast(),
            );

            raw_window_handle::WindowHandle::borrow_raw(raw_window_handle::RawWindowHandle::Gbm(
                gbm_surface_handle,
            ))
        })
    }
}

impl raw_window_handle::HasDisplayHandle for GbmDisplay {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(unsafe {
            let gbm_display_handle = raw_window_handle::GbmDisplayHandle::new(
                std::ptr::NonNull::from(&*self.gbm_device.as_raw()).cast(),
            );

            raw_window_handle::DisplayHandle::borrow_raw(raw_window_handle::RawDisplayHandle::Gbm(
                gbm_display_handle,
            ))
        })
    }
}

// Workaround for https://github.com/Smithay/gbm.rs/issues/36 : A wrapper around gbm_sys::gbm_bo
// so that we can call gbm_sys::gbm_surface_lock_front_buffer() directly.
mod gbm_rs_workaround {
    use super::OwnedFramebufferHandle;

    pub(super) struct GbmBo {
        pub(super) bo: *mut gbm_sys::gbm_bo,
        pub(super) surface: *mut gbm_sys::gbm_surface,
    }

    impl Drop for GbmBo {
        fn drop(&mut self) {
            unsafe {
                gbm_sys::gbm_surface_release_buffer(self.surface, self.bo);
            }
        }
    }

    impl GbmBo {
        pub(super) fn set_userdata(
            &mut self,
            fb: OwnedFramebufferHandle,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // Take old user data first
            let old_userdata = unsafe { gbm_sys::gbm_bo_get_user_data(self.bo) };

            unsafe extern "C" fn destroy_helper(
                _: *mut gbm_sys::gbm_bo,
                user_data: *mut std::ffi::c_void,
            ) {
                let fb_raw = user_data as *mut OwnedFramebufferHandle;
                drop(Box::from_raw(fb_raw));
            }

            let boxed_fb = Box::new(fb);
            unsafe {
                gbm_sys::gbm_bo_set_user_data(
                    self.bo,
                    Box::into_raw(boxed_fb) as _,
                    Some(destroy_helper),
                )
            }

            if !old_userdata.is_null() {
                drop(unsafe { Box::from_raw(old_userdata as *mut OwnedFramebufferHandle) });
            }

            Ok(())
        }
    }

    impl drm::buffer::Buffer for GbmBo {
        fn size(&self) -> (u32, u32) {
            unsafe { (gbm_sys::gbm_bo_get_width(self.bo), gbm_sys::gbm_bo_get_height(self.bo)) }
        }

        fn format(&self) -> gbm::Format {
            unsafe { gbm_sys::gbm_bo_get_format(self.bo) }
                .try_into()
                .expect("gbm_bo_get_format returned invalid format")
        }

        fn pitch(&self) -> u32 {
            unsafe { gbm_sys::gbm_bo_get_stride(self.bo) }
        }

        fn handle(&self) -> drm::buffer::Handle {
            unsafe {
                drm::buffer::Handle::from(std::num::NonZeroU32::new_unchecked(
                    gbm_sys::gbm_bo_get_handle(self.bo).u32_,
                ))
            }
        }
    }

    impl drm::buffer::PlanarBuffer for GbmBo {
        fn size(&self) -> (u32, u32) {
            unsafe { (gbm_sys::gbm_bo_get_width(self.bo), gbm_sys::gbm_bo_get_height(self.bo)) }
        }

        fn format(&self) -> gbm::Format {
            unsafe { gbm_sys::gbm_bo_get_format(self.bo) }
                .try_into()
                .expect("gbm_bo_get_format returned invalid format")
        }

        fn pitches(&self) -> [u32; 4] {
            let mut pitches = [0, 0, 0, 0];
            let planes = unsafe { gbm_sys::gbm_bo_get_plane_count(self.bo) };

            for i in 0..planes {
                let pitch = unsafe { gbm_sys::gbm_bo_get_stride_for_plane(self.bo, i) };
                pitches[i as usize] = pitch;
            }

            pitches
        }

        fn handles(&self) -> [Option<drm::buffer::Handle>; 4] {
            let mut handles = [None, None, None, None];
            let planes = unsafe { gbm_sys::gbm_bo_get_plane_count(self.bo) };

            for i in 0..planes {
                let handle = unsafe { gbm_sys::gbm_bo_get_handle_for_plane(self.bo, i) };
                handles[i as usize] = Some(drm::buffer::Handle::from(
                    std::num::NonZeroU32::new(unsafe { handle.u32_ })
                        .expect("received invalid gbm bo plane handle"),
                ));
            }

            handles
        }

        fn offsets(&self) -> [u32; 4] {
            let mut offsets = [0, 0, 0, 0];
            let planes = unsafe { gbm_sys::gbm_bo_get_plane_count(self.bo) };

            for i in 0..planes {
                let offset = unsafe { gbm_sys::gbm_bo_get_offset(self.bo, i) };
                offsets[i as usize] = offset;
            }

            offsets
        }
    }
}
