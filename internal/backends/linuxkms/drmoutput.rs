// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use std::cell::{Cell, RefCell};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::rc::Rc;

use crate::DeviceOpener;
use drm::buffer::Buffer;
use drm::control::Device;
use i_slint_core::platform::PlatformError;

// Wrapped needed because gbm::Device<T> wants T to be sized.
#[derive(Clone)]
pub struct SharedFd(Rc<OwnedFd>);
impl AsFd for SharedFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for SharedFd {}

impl drm::control::Device for SharedFd {}

#[derive(Default)]
enum PageFlipState {
    #[default]
    NoFrameBufferPosted,
    InitialBufferPosted,
    WaitingForPageFlip {
        _buffer_to_keep_alive_until_flip: Box<dyn Buffer>,
        ready_for_next_animation_frame: Box<dyn FnOnce()>,
    },
    ReadyForNextBuffer,
}

pub struct DrmOutput {
    pub drm_device: SharedFd,
    connector: drm::control::connector::Info,
    mode: drm::control::Mode,
    crtc: drm::control::crtc::Handle,
    last_buffer: Cell<Option<Box<dyn Buffer>>>,
    page_flip_state: Rc<RefCell<PageFlipState>>,
    page_flip_event_source_registered: Cell<bool>,
}

impl DrmOutput {
    pub fn new(device_opener: &DeviceOpener) -> Result<Self, PlatformError> {
        let mut last_err = None;
        if let Ok(drm_devices) = std::fs::read_dir("/dev/dri/") {
            for device in drm_devices {
                if let Ok(device) = device.map_err(|e| format!("Error opening DRM device: {e}")) {
                    match Self::new_with_path(device_opener, &device.path()) {
                        Ok(dsp) => return Ok(dsp),
                        Err(e) => last_err = Some(e),
                    }
                }
            }
        }
        Err(last_err.unwrap_or_else(|| "Could not create an egl display".into()))
    }

    fn new_with_path(
        device_opener: &DeviceOpener,
        device: &std::path::Path,
    ) -> Result<Self, PlatformError> {
        let drm_device = SharedFd(device_opener(device)?);

        let resources = drm_device
            .resource_handles()
            .map_err(|e| format!("Error reading DRM resource handles: {e}"))?;

        let connector = if let Ok(requested_connector_name) = std::env::var("SLINT_DRM_OUTPUT") {
            let mut connectors = resources.connectors().iter().filter_map(|handle| {
                let connector = drm_device.get_connector(*handle, false).ok()?;
                let name =
                    format!("{}-{}", connector.interface().as_str(), connector.interface_id());
                let connected = connector.state() == drm::control::connector::State::Connected;
                Some((name, connector, connected))
            });

            if requested_connector_name.eq_ignore_ascii_case("list") {
                let names_and_status = connectors
                    .map(|(name, _, connected)| format!("{} (connected: {})", name, connected))
                    .collect::<Vec<_>>();
                // Can't return error here because newlines are escaped.
                eprintln!("\nDRM Output List Requested:\n{}\nPlease select an output with the SLINT_DRM_OUTPUT environment variable and re-run the program.", names_and_status.join("\n"));
                std::process::exit(1);
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
                    (connector.state() == drm::control::connector::State::Connected)
                        .then(|| connector)
                })
                .ok_or_else(|| format!("No connected display connector found"))?
        };

        let mode = std::env::var("SLINT_DRM_MODE").map_or_else(
            |_| {
                connector
                    .modes()
                    .iter()
                    .max_by(|current_mode, next_mode| {
                        let current = (
                            current_mode
                                .mode_type()
                                .contains(drm::control::ModeTypeFlags::PREFERRED),
                            current_mode.size().0 as u32 * current_mode.size().1 as u32,
                        );
                        let next = (
                            next_mode.mode_type().contains(drm::control::ModeTypeFlags::PREFERRED),
                            next_mode.size().0 as u32 * next_mode.size().1 as u32,
                        );

                        current.cmp(&next)
                    })
                    .cloned()
                    .ok_or_else(|| format!("No preferred or non-zero size display mode found"))
            },
            |mode_str| {
                let mut modes_and_index = connector.modes().iter().cloned().enumerate();

                if mode_str.to_lowercase() == "list" {
                    let mode_names: Vec<String> = modes_and_index
                        .map(|(index, mode)| {
                            let (width, height) = mode.size();
                            format!(
                                "Index: {index} Width: {width} Height: {height} Refresh Rate: {}",
                                mode.vrefresh()
                            )
                        })
                        .collect();

                    // Can't return error here because newlines are escaped.
                    eprintln!("DRM Mode List Requested:\n{}\nPlease select a mode with the SLINT_DRM_MODE environment variable and re-run the program.", mode_names.join("\n"));
                    std::process::exit(1);
                }
                let mode_index: usize =
                    mode_str.parse().map_err(|_| format!("Invalid mode index {mode_str}"))?;
                modes_and_index.nth(mode_index).map_or_else(
                    || Err(format!("Mode index is out of bounds: {mode_index}")),
                    |(_, mode)| Ok(mode),
                )
            },
        )?;

        let encoder = connector
            .current_encoder()
            .filter(|current| connector.encoders().iter().any(|h| *h == *current))
            .and_then(|current| drm_device.get_encoder(current).ok());

        let crtc = if let Some(encoder) = encoder {
            encoder.crtc().ok_or_else(|| format!("no crtc for encoder"))?
        } else {
            // No crtc found for current encoder? Pick the first possible crtc
            // as described in https://manpages.debian.org/testing/libdrm-dev/drm-kms.7.en.html#CRTC/Encoder_Selection
            connector
                .encoders()
                .iter()
                .filter_map(|handle| drm_device.get_encoder(*handle).ok())
                .flat_map(|encoder| resources.filter_crtcs(encoder.possible_crtcs()))
                .find(|crtc_handle| drm_device.get_crtc(*crtc_handle).is_ok())
                .ok_or_else(|| {
                    format!(
                        "Could not find any crtc for any encoder connected to output {}-{}",
                        connector.interface().as_str(),
                        connector.interface_id()
                    )
                })?
        };

        //eprintln!("mode {}/{}", width, height);

        Ok(Self {
            drm_device,
            connector,
            mode,
            crtc,
            last_buffer: Cell::default(),
            page_flip_state: Default::default(),
            page_flip_event_source_registered: Cell::new(false),
        })
    }

    pub fn present(
        &self,
        front_buffer: impl Buffer + 'static,
        framebuffer_handle: drm::control::framebuffer::Handle,
        ready_for_next_animation_frame: Box<dyn FnOnce()>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(last_buffer) = self.last_buffer.replace(Some(Box::new(front_buffer))) {
            self.drm_device
                .page_flip(self.crtc, framebuffer_handle, drm::control::PageFlipFlags::EVENT, None)
                .map_err(|e| format!("Error presenting fb: {e}"))?;

            *self.page_flip_state.borrow_mut() = PageFlipState::WaitingForPageFlip {
                _buffer_to_keep_alive_until_flip: last_buffer,
                ready_for_next_animation_frame,
            };
        } else {
            self.drm_device
                .set_crtc(
                    self.crtc,
                    Some(framebuffer_handle),
                    (0, 0),
                    &[self.connector.handle()],
                    Some(self.mode),
                )
                .map_err(|e| format!("Error presenting fb: {e}"))?;
            *self.page_flip_state.borrow_mut() = PageFlipState::InitialBufferPosted;

            // We can render the next frame right away, if needed, since we have at least two buffers. The callback
            // will decide (will check if animation is running). However invoke the callback through the event loop
            // instead of directly, so that if it decides to set `needs_redraw` to true, the event loop will process it.
            i_slint_core::timers::Timer::single_shot(std::time::Duration::default(), move || {
                ready_for_next_animation_frame();
            })
        }

        Ok(())
    }

    pub fn register_page_flip_handler(
        &self,
        event_loop_handle: crate::calloop_backend::EventLoopHandle,
    ) -> Result<(), PlatformError> {
        if self.page_flip_event_source_registered.replace(true) {
            return Ok(());
        }

        let source = calloop::generic::Generic::new_with_error::<drm::SystemError>(
            self.drm_device.0.clone(),
            calloop::Interest::READ,
            calloop::Mode::Level,
        );

        event_loop_handle
            .insert_source(source, {
                let drm_device = self.drm_device.clone();
                let page_flip_state = self.page_flip_state.clone();

                move |_, _, _| {
                    if drm_device
                        .receive_events()?
                        .any(|event| matches!(event, drm::control::Event::PageFlip(..)))
                    {
                        if let PageFlipState::WaitingForPageFlip {
                            ready_for_next_animation_frame,
                            ..
                        } = page_flip_state.replace(PageFlipState::ReadyForNextBuffer)
                        {
                            ready_for_next_animation_frame();
                        }
                    }
                    Ok(calloop::PostAction::Continue)
                }
            })
            .map_err(|e| {
                PlatformError::Other(format!("Error registering page flip handler: {e}"))
            })?;
        Ok(())
    }

    pub fn is_ready_to_present(&self) -> bool {
        matches!(
            *self.page_flip_state.borrow(),
            PageFlipState::NoFrameBufferPosted
                | PageFlipState::InitialBufferPosted
                | PageFlipState::ReadyForNextBuffer
        )
    }

    pub fn size(&self) -> (u32, u32) {
        let (width, height) = self.mode.size();
        (width as u32, height as u32)
    }
}
