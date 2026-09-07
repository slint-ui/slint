// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore CLOEXEC GETFL NOCTTY NONBLOCK dlm dlmclient libdlmclient
use std::cell::RefCell;
#[cfg(not(feature = "libseat"))]
use std::fs::OpenOptions;
#[cfg(any(feature = "libseat", feature = "drm-lease"))]
use std::os::fd::AsFd;
use std::os::fd::OwnedFd;
#[cfg(feature = "libseat")]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(not(feature = "libseat"))]
use std::os::unix::fs::OpenOptionsExt;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use calloop::EventLoop;
use i_slint_core::platform::PlatformError;

use crate::BackendBuilder;
use crate::fullscreenwindowadapter::FullscreenWindowAdapter;

#[cfg(all(
    feature = "libinput",
    not(any(target_family = "windows", target_vendor = "apple", target_arch = "wasm32"))
))]
mod input;

#[derive(Clone)]
struct Proxy {
    loop_signal: Arc<Mutex<Option<calloop::LoopSignal>>>,
    quit_loop: Arc<AtomicBool>,
    user_event_channel: Arc<Mutex<calloop::channel::Sender<Box<dyn FnOnce() + Send>>>>,
}

impl Proxy {
    fn new(event_channel: calloop::channel::Sender<Box<dyn FnOnce() + Send>>) -> Self {
        Self {
            loop_signal: Arc::new(Mutex::new(None)),
            quit_loop: Arc::new(AtomicBool::new(false)),
            user_event_channel: Arc::new(Mutex::new(event_channel)),
        }
    }
}

impl i_slint_core::platform::EventLoopProxy for Proxy {
    fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
        let signal = self.loop_signal.lock().unwrap();
        signal.as_ref().map_or_else(
            || Err(i_slint_core::api::EventLoopError::EventLoopTerminated),
            |signal| {
                self.quit_loop.store(true, std::sync::atomic::Ordering::Release);
                signal.wakeup();
                Ok(())
            },
        )
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), i_slint_core::api::EventLoopError> {
        let user_event_channel = self.user_event_channel.lock().unwrap();
        user_event_channel
            .send(event)
            .map_err(|_| i_slint_core::api::EventLoopError::EventLoopTerminated)
    }
}

pub struct Backend {
    context: std::cell::OnceCell<i_slint_core::SlintContextWeak>,
    #[cfg(feature = "libseat")]
    seat: Rc<RefCell<libseat::Seat>>,
    window: RefCell<Option<Rc<FullscreenWindowAdapter>>>,
    user_event_receiver: RefCell<Option<calloop::channel::Channel<Box<dyn FnOnce() + Send>>>>,
    proxy: Proxy,
    renderer_factory:
        fn(
            &crate::DeviceOpener,
            Option<&i_slint_core::graphics::RequestedGraphicsAPI>,
        )
            -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError>,
    requested_graphics_api: Option<i_slint_core::graphics::RequestedGraphicsAPI>,
    sel_clipboard: RefCell<Option<String>>,
    clipboard: RefCell<Option<String>>,
    #[cfg(feature = "libinput")]
    libinput_event_hook: Option<Box<dyn Fn(&::input::Event) -> bool>>,
    #[cfg(feature = "drm-lease")]
    drm_lease_fd: Option<Rc<OwnedFd>>,
}

// Pick the lease fd: builder, then SLINT_DRM_LEASE_FD, then DRM_LEASE_NAME.
// A variable that is set but unusable is an error.
#[cfg(feature = "drm-lease")]
fn resolve_drm_lease_fd(builder_fd: Option<OwnedFd>) -> Result<Option<Rc<OwnedFd>>, String> {
    let fd = match builder_fd {
        Some(fd) => fd,
        None => match drm_lease_fd_from_env()? {
            Some(fd) => fd,
            None => match drm_lease_fd_from_manager()? {
                Some(fd) => fd,
                None => return Ok(None),
            },
        },
    };
    // Page flip polling needs a blocking fd.
    let flags = nix::fcntl::fcntl(fd.as_fd(), nix::fcntl::FcntlArg::F_GETFL)
        .map_err(|e| format!("Error getting DRM lease fd flags: {e}"))?;
    let mut flags = nix::fcntl::OFlag::from_bits_retain(flags);
    flags.remove(nix::fcntl::OFlag::O_NONBLOCK);
    nix::fcntl::fcntl(fd.as_fd(), nix::fcntl::FcntlArg::F_SETFL(flags))
        .map_err(|e| format!("Error making DRM lease fd blocking: {e}"))?;
    Ok(Some(Rc::new(fd)))
}

// Lease fd inherited from a launcher. Ok(None) when the variable is unset.
#[cfg(feature = "drm-lease")]
fn drm_lease_fd_from_env() -> Result<Option<OwnedFd>, String> {
    use std::os::fd::FromRawFd;
    let Ok(value) = std::env::var("SLINT_DRM_LEASE_FD") else { return Ok(None) };
    match value.trim().parse::<std::os::fd::RawFd>() {
        // Safety: the launcher promises an open fd that we now own.
        Ok(raw) if raw >= 0 => Ok(Some(unsafe { OwnedFd::from_raw_fd(raw) })),
        _ => Err(format!("SLINT_DRM_LEASE_FD {value:?} is not a file descriptor number")),
    }
}

// Ask the AGL drm-lease-manager for the lease named by DRM_LEASE_NAME.
// libdlmclient is loaded at run time so only systems with the manager need it.
// Ok(None) when the variable is unset.
#[cfg(feature = "drm-lease")]
fn drm_lease_fd_from_manager() -> Result<Option<OwnedFd>, String> {
    use std::os::fd::FromRawFd;
    use std::os::raw::{c_char, c_int};

    let Ok(name) = std::env::var("DRM_LEASE_NAME") else { return Ok(None) };
    let c_name = std::ffi::CString::new(name.as_str())
        .map_err(|_| format!("DRM_LEASE_NAME {name:?} contains a NUL byte"))?;

    #[repr(C)]
    struct DlmLease {
        _opaque: [u8; 0],
    }
    type DlmGetLease = unsafe extern "C" fn(*const c_char) -> *mut DlmLease;
    type DlmLeaseFd = unsafe extern "C" fn(*mut DlmLease) -> c_int;

    // Safety: dlopen runs the library's initializers.
    let library = ["libdlmclient.so.0", "libdlmclient.so"]
        .iter()
        .find_map(|file_name| unsafe { libloading::Library::new(file_name) }.ok())
        .ok_or_else(|| {
            format!(
                "DRM_LEASE_NAME is set to {name:?} but libdlmclient, the drm-lease-manager client library, could not be loaded"
            )
        })?;

    // Safety: the signatures match dlmclient.h.
    let raw = unsafe {
        let get_lease: libloading::Symbol<DlmGetLease> = library
            .get(b"dlm_get_lease\0")
            .map_err(|e| format!("libdlmclient has no dlm_get_lease: {e}"))?;
        let lease_fd: libloading::Symbol<DlmLeaseFd> = library
            .get(b"dlm_lease_fd\0")
            .map_err(|e| format!("libdlmclient has no dlm_lease_fd: {e}"))?;
        let lease = get_lease(c_name.as_ptr());
        if lease.is_null() {
            return Err(format!(
                "drm-lease-manager did not grant the lease {name:?}: {}",
                std::io::Error::last_os_error()
            ));
        }
        lease_fd(lease)
    };
    if raw < 0 {
        return Err(format!("drm-lease-manager returned no file descriptor for lease {name:?}"));
    }

    // dlm_release_lease() closes the fd, so the lease and the library are kept for the whole
    // process.
    std::mem::forget(library);
    // Safety: the fd stays open because the lease is never released.
    Ok(Some(unsafe { OwnedFd::from_raw_fd(raw) }))
}

impl Backend {
    pub fn build(builder: BackendBuilder) -> Result<Self, PlatformError> {
        let (user_event_sender, user_event_receiver) = calloop::channel::channel();

        let renderer_factory = match builder.renderer_name.as_deref() {
            #[cfg(enable_skia_wgpu)]
            Some("skia-vulkan") | Some("skia-wgpu") => {
                crate::renderer::skia::SkiaRendererAdapter::new_wgpu
            }
            #[cfg(feature = "renderer-skia-opengl")]
            Some("skia-opengl") => crate::renderer::skia::SkiaRendererAdapter::new_opengl,
            #[cfg(enable_skia)]
            Some("skia-software") => crate::renderer::skia::SkiaRendererAdapter::new_software,
            #[cfg(feature = "renderer-femtovg")]
            Some("femtovg") => crate::renderer::femtovg::FemtoVGRendererAdapter::new,
            #[cfg(feature = "renderer-femtovg-wgpu")]
            Some("femtovg-wgpu") => crate::renderer::femtovg_wgpu::FemtoVGWgpuRendererAdapter::new,
            #[cfg(feature = "renderer-software")]
            Some("software") => crate::renderer::sw::SoftwareRendererAdapter::new,
            #[cfg(feature = "renderer-vello")]
            Some("vello") => crate::renderer::vello::VelloRendererAdapter::new,
            None => crate::renderer::try_skia_then_femtovg_then_software,
            Some(renderer_name) => {
                eprintln!(
                    "slint linuxkms backend: unrecognized renderer {}, falling back default",
                    renderer_name
                );
                crate::renderer::try_skia_then_femtovg_then_software
            }
        };

        #[cfg(feature = "libseat")]
        let seat_active = Rc::new(RefCell::new(false));

        //libseat::set_log_level(libseat::LogLevel::Debug);

        #[cfg(feature = "libseat")]
        let mut seat = {
            let seat_active = seat_active.clone();
            libseat::Seat::open(move |_seat, event| match event {
                libseat::SeatEvent::Enable => {
                    *seat_active.borrow_mut() = true;
                }
                libseat::SeatEvent::Disable => {
                    unimplemented!("Seat deactivation is not implemented");
                }
            })
            .map_err(|e| format!("Error opening session with libseat: {e}"))?
        };

        #[cfg(feature = "libseat")]
        while !(*seat_active.borrow()) {
            if seat.dispatch(5000).map_err(|e| format!("Error waiting for seat activation: {e}"))?
                == 0
            {
                return Err("Timeout while waiting to activate session".to_string().into());
            }
        }

        #[cfg(feature = "drm-lease")]
        let drm_lease_fd = resolve_drm_lease_fd(builder.drm_lease_fd)?;

        Ok(Backend {
            context: Default::default(),
            #[cfg(feature = "libseat")]
            seat: Rc::new(RefCell::new(seat)),
            window: Default::default(),
            user_event_receiver: RefCell::new(Some(user_event_receiver)),
            proxy: Proxy::new(user_event_sender),
            renderer_factory,
            requested_graphics_api: builder.requested_graphics_api,
            sel_clipboard: Default::default(),
            clipboard: Default::default(),
            #[cfg(feature = "libinput")]
            libinput_event_hook: builder.libinput_event_hook,
            #[cfg(feature = "drm-lease")]
            drm_lease_fd,
        })
    }
}

impl i_slint_core::platform::Platform for Backend {
    fn bind_context(&self, ctx: i_slint_core::SlintContextWeak, _: i_slint_core::InternalToken) {
        let _ = self.context.set(ctx);
    }

    fn create_window_adapter(
        &self,
    ) -> Result<std::rc::Rc<dyn i_slint_core::window::WindowAdapter>, PlatformError> {
        #[cfg(feature = "libseat")]
        let device_accessor = |device: &std::path::Path| -> Result<Rc<OwnedFd>, PlatformError> {
            let device = self
                .seat
                .borrow_mut()
                .open_device(&device)
                .map_err(|e| format!("Error opening device {}: {e}", device.display()))?;

            // For polling for drm::control::Event::PageFlip we need a blocking FD. Would be better to do this non-blocking
            let fd = device.as_fd();
            let flags = nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_GETFL)
                .map_err(|e| format!("Error getting file descriptor flags: {e}"))?;
            let mut flags = nix::fcntl::OFlag::from_bits_retain(flags);
            flags.remove(nix::fcntl::OFlag::O_NONBLOCK);
            nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_SETFL(flags))
                .map_err(|e| format!("Error making device fd non-blocking: {e}"))?;

            // Safety: We take ownership of the now shared FD, ... although we should be using libseat's close_device....
            Ok(Rc::new(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd.as_raw_fd()) }))
        };

        #[cfg(not(feature = "libseat"))]
        let device_accessor = |device: &std::path::Path| -> Result<Rc<OwnedFd>, PlatformError> {
            let device = OpenOptions::new()
                .custom_flags((nix::fcntl::OFlag::O_NOCTTY | nix::fcntl::OFlag::O_CLOEXEC).bits())
                .read(true)
                .write(true)
                .open(device)
                .map(|file| file.into())
                .map_err(|e| format!("Error opening device {}: {e}", device.display()))?;

            Ok(Rc::new(device))
        };

        // This could be per-screen, once we support multiple outputs
        let rotation =
            std::env::var("SLINT_KMS_ROTATION").map_or(Ok(Default::default()), |rot_str| {
                rot_str
                    .as_str()
                    .try_into()
                    .map_err(|e| format!("Failed to parse SLINT_KMS_ROTATION: {e}"))
            })?;

        let device_opener = crate::DeviceOpener::new(
            device_accessor,
            #[cfg(feature = "drm-lease")]
            self.drm_lease_fd.clone(),
        );

        let renderer =
            (self.renderer_factory)(&device_opener, self.requested_graphics_api.as_ref())?;
        let adapter = FullscreenWindowAdapter::new(renderer, rotation)?;

        *self.window.borrow_mut() = Some(adapter.clone());

        Ok(adapter)
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        let mut event_loop: EventLoop<LoopData> =
            EventLoop::try_new().map_err(|e| format!("Error creating event loop: {}", e))?;

        let loop_signal = event_loop.get_signal();

        *self.proxy.loop_signal.lock().unwrap() = Some(loop_signal.clone());
        if let Some(adapter) = self.window.borrow().as_ref() {
            adapter.set_loop_signal(loop_signal.clone());
        }
        let quit_loop = self.proxy.quit_loop.clone();

        #[cfg(feature = "libinput")]
        let mouse_position_property = input::LibInputHandler::init(
            &self.window,
            &event_loop.handle(),
            #[cfg(feature = "libseat")]
            &self.seat,
            &self.libinput_event_hook,
        )?;

        // Without libinput there is no pointer to track, so the cursor property
        // stays empty for the lifetime of the loop.
        #[cfg(not(feature = "libinput"))]
        let mouse_position_property = Rc::pin(i_slint_core::Property::<
            Option<i_slint_core::api::LogicalPosition>,
        >::new(None));

        let Some(user_event_receiver) = self.user_event_receiver.borrow_mut().take() else {
            return Err("Re-entering the linuxkms event loop is currently not supported"
                .to_string()
                .into());
        };

        let callbacks_to_invoke_per_iteration = Rc::new(RefCell::new(Vec::new()));

        event_loop
            .handle()
            .insert_source(user_event_receiver, {
                let callbacks_to_invoke_per_iteration = callbacks_to_invoke_per_iteration.clone();
                move |event, _, _| {
                    let calloop::channel::Event::Msg(callback) = event else { return };
                    // Remember the callbacks and invoke them after updating the animation tick
                    callbacks_to_invoke_per_iteration.borrow_mut().push(callback);
                }
            })
            .map_err(
                |e: calloop::InsertError<calloop::channel::Channel<Box<dyn FnOnce() + Send>>>| {
                    format!("Error registering user event channel source: {e}")
                },
            )?;

        let mut loop_data = LoopData::default();

        quit_loop.store(false, std::sync::atomic::Ordering::Release);

        let ctx = self
            .context
            .get()
            .and_then(|ctx| ctx.upgrade())
            .expect("the event loop runs inside the context that owns this backend");

        while !quit_loop.load(std::sync::atomic::Ordering::Acquire) {
            ctx.update_timers_and_animations();

            // Only after updating the animation tick, invoke callbacks from invoke_from_event_loop(). They
            // might set animated properties, which requires an up-to-date start time.
            for callback in callbacks_to_invoke_per_iteration.take().into_iter() {
                callback();
            }

            if let Some(adapter) = self.window.borrow().as_ref() {
                adapter.clone().render_if_needed(mouse_position_property.as_ref())?;
            };

            let next_timeout = ctx.duration_until_next_timer_update();
            event_loop
                .dispatch(next_timeout, &mut loop_data)
                .map_err(|e| format!("Error dispatch events: {e}"))?;
        }

        Ok(())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
        Some(Box::new(self.proxy.clone()))
    }

    fn clipboard_text(&self, clipboard: i_slint_core::platform::Clipboard) -> Option<String> {
        match clipboard {
            i_slint_core::platform::Clipboard::DefaultClipboard => self.clipboard.borrow().clone(),
            i_slint_core::platform::Clipboard::SelectionClipboard => {
                self.sel_clipboard.borrow().clone()
            }
            _ => None,
        }
    }
    fn set_clipboard_text(&self, text: &str, clipboard: i_slint_core::platform::Clipboard) {
        match clipboard {
            i_slint_core::platform::Clipboard::DefaultClipboard => {
                *self.clipboard.borrow_mut() = Some(text.into())
            }
            i_slint_core::platform::Clipboard::SelectionClipboard => {
                *self.sel_clipboard.borrow_mut() = Some(text.into())
            }
            _ => (),
        }
    }
}

#[derive(Default)]
pub struct LoopData {}
