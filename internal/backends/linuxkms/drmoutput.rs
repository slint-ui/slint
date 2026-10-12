// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore CRTC crtcs htotal modeset NONBLOCK vrefresh vtotal
use std::cell::{Cell, RefCell};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::rc::Rc;

use crate::DeviceOpener;
use drm::Device as DrmDevice;
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

impl DrmDevice for SharedFd {}

#[cfg(any(feature = "renderer-skia-opengl", feature = "renderer-femtovg", gbm_dmabuf))]
pub struct OwnedFramebufferHandle {
    pub handle: drm::control::framebuffer::Handle,
    pub device: SharedFd,
}

#[cfg(any(feature = "renderer-skia-opengl", feature = "renderer-femtovg", gbm_dmabuf))]
impl Drop for OwnedFramebufferHandle {
    fn drop(&mut self) {
        self.device.destroy_framebuffer(self.handle).ok();
    }
}

impl drm::control::Device for SharedFd {}

#[derive(Default)]
enum PageFlipState {
    #[default]
    NoFrameBufferPosted,
    InitialBufferPosted,
    WaitingForPageFlip {
        /// `None` when the caller owns the buffers for the lifetime of the
        /// output, as [`DrmOutput::present_framebuffer`]'s callers do.
        _buffer_to_keep_alive_until_flip: Option<Box<dyn Buffer>>,
    },
    ReadyForNextBuffer,
}

pub struct DrmOutput {
    pub drm_device: SharedFd,
    pub connector: drm::control::connector::Info,
    mode: drm::control::Mode,
    crtc: drm::control::crtc::Handle,
    last_buffer: Cell<Option<Box<dyn Buffer>>>,
    page_flip_state: Rc<RefCell<PageFlipState>>,
    /// See [`Self::present_framebuffer_after`].
    #[cfg(skia_wgpu_30)]
    fenced_flips: Cell<FencedFlips>,
}

#[cfg(skia_wgpu_30)]
#[derive(Clone, Copy, Default)]
enum FencedFlips {
    /// Not looked up yet.
    #[default]
    Unknown,
    /// The display can't wait for a sync file,
    /// or failed to [`FENCED_FLIP_ATTEMPTS`] times in a row.
    Unsupported,
    Supported {
        flip: FencedFlip,
        failures_in_a_row: u8,
    },
}

#[cfg(skia_wgpu_30)]
const FENCED_FLIP_ATTEMPTS: u8 = 3;

/// The primary plane's properties an atomic commit sets to flip once a sync file signaled.
#[cfg(skia_wgpu_30)]
#[derive(Clone, Copy)]
struct FencedFlip {
    plane: drm::control::plane::Handle,
    framebuffer: drm::control::property::Handle,
    in_fence: drm::control::property::Handle,
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
                eprintln!(
                    "\nDRM Output List Requested:\n{}\nPlease select an output with the SLINT_DRM_OUTPUT environment variable and re-run the program.",
                    names_and_status.join("\n")
                );
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
                        .then_some(connector)
                })
                .ok_or_else(|| "No connected display connector found".to_string())?
        };

        let mode = std::env::var("SLINT_DRM_MODE").map_or_else(
            |_| {
                connector
                    .modes()
                    .iter()
                    // Displays often list their native size several times, down to the
                    // 24 Hz of film, and don't always mark one of them as preferred.
                    .max_by_key(|mode| {
                        (
                            mode.mode_type().contains(drm::control::ModeTypeFlags::PREFERRED),
                            mode.size().0 as u32 * mode.size().1 as u32,
                            refresh_rate_millihertz(mode),
                        )
                    })
                    .cloned()
                    .ok_or_else(|| "No preferred or non-zero size display mode found".to_string())
            },
            |mode_str| {
                let modes_and_index = connector.modes().iter().cloned().enumerate();

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
                select_requested_mode(connector.modes(), &mode_str)
            },
        )?;

        let encoder = connector
            .current_encoder()
            .filter(|current| connector.encoders().contains(current))
            .and_then(|current| drm_device.get_encoder(current).ok());

        let crtc = if let Some(encoder) = encoder {
            encoder.crtc().ok_or_else(|| "no crtc for encoder".to_string())?
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
            #[cfg(skia_wgpu_30)]
            fenced_flips: Default::default(),
        })
    }

    /// Posts `framebuffer_handle` and keeps `front_buffer` alive until the flip completes.
    /// Use this when the buffer is allocated per frame;
    /// see [`Self::present_framebuffer`] when the caller owns a persistent buffer ring.
    pub fn present(
        &self,
        front_buffer: impl Buffer + 'static,
        framebuffer_handle: drm::control::framebuffer::Handle,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // The buffer this one replaces is the one still on screen until the flip
        // completes, so that is the one to keep alive.
        let last_buffer = self.last_buffer.replace(Some(Box::new(front_buffer)));
        self.post(framebuffer_handle, last_buffer)
    }

    /// Posts `framebuffer_handle` without taking ownership of its buffer.
    /// The caller must keep the buffer alive at least until the flip completes.
    #[cfg(gbm_dmabuf)]
    pub fn present_framebuffer(
        &self,
        framebuffer_handle: drm::control::framebuffer::Handle,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.post(framebuffer_handle, None)
    }

    /// Posts `framebuffer_handle` once `sync_file` signaled, without waiting for it:
    /// the display waits instead.
    /// Waits for the last flip to land first.
    /// Returns `false` where it can't, without posting.
    /// The caller then waits for the GPU and calls [`Self::present_framebuffer`].
    ///
    /// The caller must keep the buffer alive at least until the flip completes.
    #[cfg(skia_wgpu_30)]
    pub fn present_framebuffer_after(
        &self,
        framebuffer_handle: drm::control::framebuffer::Handle,
        sync_file: &OwnedFd,
    ) -> bool {
        use std::os::fd::AsRawFd;
        if matches!(*self.page_flip_state.borrow(), PageFlipState::NoFrameBufferPosted) {
            return false;
        }
        let FencedFlips::Supported { flip, failures_in_a_row } = self.fenced_flips() else {
            return false;
        };
        self.wait_for_page_flip();
        let mut request = drm::control::atomic::AtomicModeReq::new();
        request.add_property(
            flip.plane,
            flip.framebuffer,
            drm::control::property::Value::Framebuffer(Some(framebuffer_handle)),
        );
        // The kernel takes its own reference to the fence.
        request.add_property(
            flip.plane,
            flip.in_fence,
            drm::control::property::Value::SignedRange(sync_file.as_raw_fd().into()),
        );
        if let Err(e) = self.drm_device.atomic_commit(
            drm::control::AtomicCommitFlags::NONBLOCK
                | drm::control::AtomicCommitFlags::PAGE_FLIP_EVENT,
            request,
        ) {
            let failures_in_a_row = failures_in_a_row + 1;
            if failures_in_a_row < FENCED_FLIP_ATTEMPTS {
                self.fenced_flips.set(FencedFlips::Supported { flip, failures_in_a_row });
            } else {
                eprintln!(
                    "Error presenting frames with an in-fence, so the CPU waits for the GPU: {e}"
                );
                self.fenced_flips.set(FencedFlips::Unsupported);
                let _ = self.drm_device.set_client_capability(drm::ClientCapability::Atomic, false);
            }
            return false;
        }
        self.fenced_flips.set(FencedFlips::Supported { flip, failures_in_a_row: 0 });
        *self.page_flip_state.borrow_mut() =
            PageFlipState::WaitingForPageFlip { _buffer_to_keep_alive_until_flip: None };
        true
    }

    #[cfg(skia_wgpu_30)]
    pub fn takes_sync_files(&self) -> bool {
        // The first frame sets the mode, see `post`.
        !matches!(*self.page_flip_state.borrow(), PageFlipState::NoFrameBufferPosted)
            && matches!(self.fenced_flips(), FencedFlips::Supported { .. })
    }

    #[cfg(skia_wgpu_30)]
    fn fenced_flips(&self) -> FencedFlips {
        if let FencedFlips::Unknown = self.fenced_flips.get() {
            self.fenced_flips.set(match self.find_fenced_flip() {
                Some(flip) => FencedFlips::Supported { flip, failures_in_a_row: 0 },
                None => FencedFlips::Unsupported,
            });
        }
        self.fenced_flips.get()
    }

    /// The primary plane of the CRTC and its properties for a fenced flip,
    /// where the driver takes atomic commits.
    #[cfg(skia_wgpu_30)]
    fn find_fenced_flip(&self) -> Option<FencedFlip> {
        let planes = self
            .drm_device
            .set_client_capability(drm::ClientCapability::Atomic, true)
            .and_then(|()| self.drm_device.plane_handles())
            .unwrap_or_default();
        let flip = planes.into_iter().find_map(|plane| {
            if self.drm_device.get_plane(plane).ok()?.crtc() != Some(self.crtc) {
                return None;
            }
            let values = self.drm_device.get_properties(plane).ok()?;
            let properties = values.as_hashmap(&self.drm_device).ok()?;
            let type_handle = properties.get("type")?.handle();
            let primary = values.iter().any(|(handle, value)| {
                *handle == type_handle && *value == drm::control::PlaneType::Primary as u32 as u64
            });
            primary.then_some(FencedFlip {
                plane,
                framebuffer: properties.get("FB_ID")?.handle(),
                in_fence: properties.get("IN_FENCE_FD")?.handle(),
            })
        });
        if flip.is_none() {
            let _ = self.drm_device.set_client_capability(drm::ClientCapability::Atomic, false);
            eprintln!(
                "The display takes no atomic commit with an in-fence, so the CPU waits for the GPU"
            );
        }
        flip
    }

    fn post(
        &self,
        framebuffer_handle: drm::control::framebuffer::Handle,
        buffer_to_keep_alive: Option<Box<dyn Buffer>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !matches!(*self.page_flip_state.borrow(), PageFlipState::NoFrameBufferPosted) {
            self.drm_device
                .page_flip(self.crtc, framebuffer_handle, drm::control::PageFlipFlags::EVENT, None)
                .map_err(|e| format!("Error presenting framebuffer on screen: {e}"))?;

            *self.page_flip_state.borrow_mut() = PageFlipState::WaitingForPageFlip {
                _buffer_to_keep_alive_until_flip: buffer_to_keep_alive,
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
                .map_err(|e| format!("Error presenting framebuffer on screen: {e}"))?;
            *self.page_flip_state.borrow_mut() = PageFlipState::InitialBufferPosted;
        }

        Ok(())
    }

    pub fn wait_for_page_flip(&self) {
        if matches!(
            *self.page_flip_state.borrow(),
            PageFlipState::NoFrameBufferPosted
                | PageFlipState::InitialBufferPosted
                | PageFlipState::ReadyForNextBuffer
        ) {
            return;
        }

        loop {
            let Ok(mut event_it) = self.drm_device.receive_events() else {
                return;
            };

            if event_it.any(|event| matches!(event, drm::control::Event::PageFlip(..)))
                && let PageFlipState::WaitingForPageFlip { .. } =
                    self.page_flip_state.replace(PageFlipState::ReadyForNextBuffer)
            {
                return;
            }
        }
    }

    pub fn get_supported_formats(&self) -> Result<Vec<drm::buffer::DrmFourcc>, PlatformError> {
        // Try to set universal planes client capability if possible
        let _ = self.drm_device.set_client_capability(drm::ClientCapability::UniversalPlanes, true);

        let mut all_formats = std::collections::HashSet::new();

        // Iterate through all planes and collect formats from compatible ones
        if let Ok(plane_handles) = self.drm_device.plane_handles() {
            for &plane_handle in &plane_handles {
                if let Ok(plane) = self.drm_device.get_plane(plane_handle)
                    && plane.crtc() == Some(self.crtc)
                {
                    // Collect formats from this compatible plane
                    for &format_u32 in plane.formats() {
                        if let Ok(format) = drm::buffer::DrmFourcc::try_from(format_u32) {
                            all_formats.insert(format);
                        }
                    }
                }
            }
        }

        if all_formats.is_empty() {
            eprintln!(
                "No available formats found for any plane with CRTC {:?}. Falling back to XRGB8888 format",
                self.crtc
            );

            Ok(vec![drm::buffer::DrmFourcc::Xrgb8888])
        } else {
            Ok(all_formats.into_iter().collect())
        }
    }

    #[cfg(feature = "gbm")]
    pub fn create_gbm_device(&self) -> Result<gbm::Device<SharedFd>, PlatformError> {
        #[cfg(all(feature = "dlopen", not(test)))]
        crate::dlopen::gbm::load()?;

        gbm::Device::new(self.drm_device.clone())
            .map_err(|e| format!("Error creating gbm device: {e}").into())
    }

    pub fn size(&self) -> (u32, u32) {
        let (width, height) = self.mode.size();
        (width as u32, height as u32)
    }

    /// Returns the refresh rate in millihertz, see [`refresh_rate_millihertz`].
    #[cfg(wgpu_surface)]
    pub fn refresh_rate_millihertz(&self) -> u32 {
        refresh_rate_millihertz(&self.mode)
    }

    #[cfg(skia_wgpu_29)]
    /// Creates a wgpu-29 DRM surface target from this output.
    pub fn wgpu_29_surface_target(
        &self,
    ) -> Result<
        (i_slint_core::graphics::wgpu_29::SurfaceTarget, i_slint_core::api::PhysicalSize),
        PlatformError,
    > {
        use i_slint_core::graphics::wgpu_29::wgpu;
        use std::os::fd::AsRawFd;
        let plane = self.find_compatible_plane()?;
        let (width, height) = self.size();
        let target =
            i_slint_core::graphics::wgpu_29::SurfaceTarget::Drm(wgpu::SurfaceTargetUnsafe::Drm {
                fd: self.drm_device.as_fd().as_raw_fd(),
                plane: plane.handle().into(),
                connector_id: self.connector.handle().into(),
                width,
                height,
                refresh_rate: self.refresh_rate_millihertz(),
            });
        Ok((target, i_slint_core::api::PhysicalSize::new(width, height)))
    }

    #[cfg(wgpu_30_surface_target)]
    /// Creates a wgpu-30 DRM surface target from this output.
    pub fn wgpu_30_surface_target(
        &self,
    ) -> Result<
        (i_slint_core::graphics::wgpu_30::SurfaceTarget, i_slint_core::api::PhysicalSize),
        PlatformError,
    > {
        use i_slint_core::graphics::wgpu_30::wgpu;
        use std::os::fd::AsRawFd;
        let plane = self.find_compatible_plane()?;
        let (width, height) = self.size();
        let target =
            i_slint_core::graphics::wgpu_30::SurfaceTarget::Drm(wgpu::SurfaceTargetUnsafe::Drm {
                fd: self.drm_device.as_fd().as_raw_fd(),
                plane: plane.handle().into(),
                connector_id: self.connector.handle().into(),
                width,
                height,
                refresh_rate: self.refresh_rate_millihertz(),
            });
        Ok((target, i_slint_core::api::PhysicalSize::new(width, height)))
    }

    // Iterate through all planes and collect formats from compatible ones
    #[cfg(wgpu_surface)]
    pub fn find_compatible_plane(&self) -> Result<drm::control::plane::Info, PlatformError> {
        let _ = self.drm_device.set_client_capability(drm::ClientCapability::UniversalPlanes, true);
        let plane_handles = self
            .drm_device
            .plane_handles()
            .map_err(|e| format!("Error obtaining drm plane handles: {e}"))?
            .into_iter()
            .filter(|plane_handle| {
                let Ok(plane_info) = self.drm_device.get_plane(*plane_handle) else {
                    return false;
                };
                self.drm_device
                    .resource_handles()
                    .unwrap()
                    .filter_crtcs(plane_info.possible_crtcs())
                    .contains(&self.crtc)
            });

        for plane_handle in plane_handles {
            if let Ok(plane) = self.drm_device.get_plane(plane_handle) {
                return Ok(plane);
            }
        }

        Err(PlatformError::Other("Could not find plane matching crtc".into()))
    }
}

/// Returns `mode`'s refresh rate in millihertz, computed from its pixel clock
/// and timing parameters. This matches the precision used by Vulkan's
/// VkDisplayModeParametersKHR::refreshRate.
fn refresh_rate_millihertz(mode: &drm::control::Mode) -> u32 {
    let clock = mode.clock() as u64; // in kHz
    let (_, _, htotal) = mode.hsync();
    let (_, _, vtotal) = mode.vsync();
    let htotal = htotal as u64;
    let vtotal = vtotal as u64;
    if htotal == 0 || vtotal == 0 {
        // Fallback to rounded vrefresh * 1000
        return mode.vrefresh() * 1000;
    }
    // clock is in kHz, so clock * 1_000_000 gives us millihertz * htotal * vtotal
    ((clock * 1_000_000 + (htotal * vtotal) / 2) / (htotal * vtotal)) as u32
}

fn select_requested_mode(
    modes: &[drm::control::Mode],
    mode_str: &str,
) -> Result<drm::control::Mode, String> {
    if let Ok(mode_index) = mode_str.parse::<usize>() {
        return modes
            .get(mode_index)
            .copied()
            .map_or_else(|| Err(format!("Mode index is out of bounds: {mode_index}")), Ok);
    }

    let (width, height) = mode_str.split_once('x').ok_or_else(|| {
        format!(
            "Invalid SLINT_DRM_MODE value '{mode_str}': expected a mode index, WIDTHxHEIGHT, or 'list'"
        )
    })?;
    let requested_size = (
        width
            .parse::<u16>()
            .map_err(|_| format!("Invalid width in SLINT_DRM_MODE value '{mode_str}'"))?,
        height
            .parse::<u16>()
            .map_err(|_| format!("Invalid height in SLINT_DRM_MODE value '{mode_str}'"))?,
    );

    modes
        .iter()
        .filter(|mode| mode.size() == requested_size)
        .max_by_key(|mode| {
            (
                mode.mode_type().contains(drm::control::ModeTypeFlags::PREFERRED),
                refresh_rate_millihertz(mode),
            )
        })
        .cloned()
        .ok_or_else(|| {
            format!("Requested DRM mode {}x{} is not available", requested_size.0, requested_size.1)
        })
}

#[cfg(test)]
mod mode_tests;
