// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::cell::Cell;
use std::os::fd::{AsFd, BorrowedFd};
use std::sync::Arc;

use crate::DeviceOpener;
use drm::control::Device;
use gbm::AsRaw;
use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;

// Wrapped needed because gbm::Device<T> wants T to be sized.
#[derive(Clone)]
pub struct SharedFd(Arc<dyn AsFd>);
impl AsFd for SharedFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for SharedFd {}

impl drm::control::Device for SharedFd {}

struct OwnedFramebufferHandle {
    handle: drm::control::framebuffer::Handle,
    device: SharedFd,
}

impl Drop for OwnedFramebufferHandle {
    fn drop(&mut self) {
        self.device.destroy_framebuffer(self.handle).ok();
    }
}

pub struct EglDisplay {
    last_buffer: Cell<Option<gbm::BufferObject<OwnedFramebufferHandle>>>,
    crtc: drm::control::crtc::Handle,
    connector: drm::control::connector::Info,
    mode: drm::control::Mode,
    gbm_surface: gbm::Surface<OwnedFramebufferHandle>,
    gbm_device: gbm::Device<SharedFd>,
    drm_device: SharedFd,
    pub size: PhysicalWindowSize,
}

impl super::Presenter for EglDisplay {
    fn present(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut front_buffer = unsafe {
            self.gbm_surface
                .lock_front_buffer()
                .map_err(|e| format!("Error locking gmb surface front buffer: {e}"))?
        };
        // TODO: respect modifiers & planes if front_buffer has, use add_planar_framebuffer and fall back to add_framebuffer
        let fb = self
            .gbm_device
            .add_framebuffer(&front_buffer, 24, 32)
            .map_err(|e| format!("Error adding gbm buffer as framebuffer: {e}"))?;
        front_buffer
            .set_userdata(OwnedFramebufferHandle { handle: fb, device: self.drm_device.clone() })
            .map_err(|e| format!("Error setting userdata on gbm surface front buffer: {e}"))?;

        if let Some(last_buffer) = self.last_buffer.replace(Some(front_buffer)) {
            self.gbm_device
                .page_flip(self.crtc, fb, drm::control::PageFlipFlags::EVENT, None)
                .map_err(|e| format!("Error presenting fb: {e}"))?;

            for event in self.gbm_device.receive_events().unwrap() {
                if matches!(event, drm::control::Event::PageFlip(..)) {
                    break;
                }
            }

            drop(last_buffer);
        } else {
            self.gbm_device
                .set_crtc(self.crtc, Some(fb), (0, 0), &[self.connector.handle()], Some(self.mode))
                .map_err(|e| format!("Error presenting fb: {e}"))?;
        }

        Ok(())
    }
}

impl raw_window_handle::HasWindowHandle for EglDisplay {
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

impl raw_window_handle::HasDisplayHandle for EglDisplay {
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

pub fn create_egl_display(device_opener: &DeviceOpener) -> Result<EglDisplay, PlatformError> {
    let mut last_err = None;
    if let Ok(drm_devices) = std::fs::read_dir("/dev/dri/") {
        for device in drm_devices {
            if let Ok(device) = device.map_err(|e| format!("Error opening DRM device: {e}")) {
                match try_create_egl_display(device_opener, &device.path()) {
                    Ok(dsp) => return Ok(dsp),
                    Err(e) => last_err = Some(e),
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| "Could not create an egl display".into()))
}

pub fn try_create_egl_display(
    device_opener: &DeviceOpener,
    device: &std::path::Path,
) -> Result<EglDisplay, PlatformError> {
    let drm_device = SharedFd(device_opener(device)?);

    let resources = drm_device
        .resource_handles()
        .map_err(|e| format!("Error reading DRM resource handles: {e}"))?;

    let connector = if let Ok(requested_connector_name) = std::env::var("SLINT_DRM_OUTPUT") {
        let mut connectors = resources.connectors().iter().filter_map(|handle| {
            let connector = drm_device.get_connector(*handle, false).ok()?;
            let name = format!("{}-{}", connector.interface().as_str(), connector.interface_id());
            let connected = connector.state() == drm::control::connector::State::Connected;
            Some((name, connector, connected))
        });

        if requested_connector_name.eq_ignore_ascii_case("list") {
            let names_and_status = connectors
                .map(|(name, _, connected)| format!("{} (connected: {})", name, connected))
                .collect::<Vec<_>>();
            // Can't return error here because newlines are escaped.
            panic!("\nDRM Output List Requested:\n{}\n", names_and_status.join("\n"));
        } else {
            let (_, connector, connected) =
                connectors.find(|(name, _, _)| name == &requested_connector_name).ok_or_else(
                    || format!("No output with the name '{}' found", requested_connector_name),
                )?;

            if !connected {
                return Err(format!(
                    "Requested output '{}' is not connected",
                    requested_connector_name
                )
                .into());
            };

            connector
        }
    } else {
        resources
            .connectors()
            .iter()
            .find_map(|handle| {
                let connector = drm_device.get_connector(*handle, false).ok()?;
                (connector.state() == drm::control::connector::State::Connected).then(|| connector)
            })
            .ok_or_else(|| format!("No connected display connector found"))?
    };

    let mode = *connector
        .modes()
        .iter()
        .max_by(|current_mode, next_mode| {
            let current = (
                current_mode.mode_type().contains(drm::control::ModeTypeFlags::PREFERRED),
                current_mode.size().0 as u32 * current_mode.size().1 as u32,
            );
            let next = (
                next_mode.mode_type().contains(drm::control::ModeTypeFlags::PREFERRED),
                next_mode.size().0 as u32 * next_mode.size().1 as u32,
            );

            current.cmp(&next)
        })
        .ok_or_else(|| format!("No preferred or non-zero size display mode found"))?;

    let encoder = connector
        .encoders()
        .iter()
        .find_map(|handle| {
            if connector.current_encoder() == Some(*handle) {
                drm_device.get_encoder(*handle).ok()
            } else {
                None
            }
        })
        .ok_or_else(|| format!("Not encoder found for connector"))?;

    let crtc = encoder.crtc().ok_or_else(|| format!("no crtc for encoder"))?;

    let (width, height) = mode.size();
    let width = std::num::NonZeroU32::new(width as _)
        .ok_or_else(|| format!("Invalid mode screen width {width}"))?;
    let height = std::num::NonZeroU32::new(height as _)
        .ok_or_else(|| format!("Invalid mode screen height {height}"))?;

    //eprintln!("mode {}/{}", width, height);

    let gbm_device = gbm::Device::new(drm_device.clone())
        .map_err(|e| format!("Error creating gbm device: {e}"))?;

    let gbm_surface = gbm_device
        .create_surface::<OwnedFramebufferHandle>(
            width.get(),
            height.get(),
            gbm::Format::Xrgb8888,
            gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING,
        )
        .map_err(|e| format!("Error creating gbm surface: {e}"))?;

    let window_size = PhysicalWindowSize::new(width.get(), height.get());

    Ok(EglDisplay {
        last_buffer: Cell::default(),
        crtc,
        connector,
        mode,
        gbm_surface,
        gbm_device,
        drm_device,
        size: window_size,
    })
}
