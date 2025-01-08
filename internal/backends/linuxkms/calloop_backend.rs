// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
#[cfg(not(feature = "libseat"))]
use std::fs::OpenOptions;
use std::os::fd::OwnedFd;
#[cfg(feature = "libseat")]
use std::os::fd::{AsFd, AsRawFd, FromRawFd};
#[cfg(not(feature = "libseat"))]
use std::os::unix::fs::OpenOptionsExt;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use calloop::EventLoop;
use i_slint_core::platform::PlatformError;

use crate::fullscreenwindowadapter::FullscreenWindowAdapter;

#[cfg(not(any(
    target_family = "windows",
    target_os = "macos",
    target_os = "ios",
    target_arch = "wasm32"
)))]
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
    #[cfg(feature = "libseat")]
    seat: Rc<RefCell<libseat::Seat>>,
    window: RefCell<Option<Rc<FullscreenWindowAdapter>>>,
    user_event_receiver: RefCell<Option<calloop::channel::Channel<Box<dyn FnOnce() + Send>>>>,
    proxy: Proxy,
    renderer_factory: for<'a> fn(
        &'a crate::DeviceOpener,
    ) -> Result<
        Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>,
        PlatformError,
    >,
    sel_clipboard: RefCell<Option<String>>,
    clipboard: RefCell<Option<String>>,
}

impl Backend {
    pub fn new() -> Result<Self, PlatformError> {
        Self::new_with_renderer_by_name(None)
    }
    pub fn new_with_renderer_by_name(renderer_name: Option<&str>) -> Result<Self, PlatformError> {
        let (user_event_sender, user_event_receiver) = calloop::channel::channel();

        let renderer_factory = match renderer_name {
            #[cfg(feature = "renderer-skia-vulkan")]
            Some("skia-vulkan") => crate::renderer::skia::SkiaRendererAdapter::new_vulkan,
            #[cfg(feature = "renderer-skia-opengl")]
            Some("skia-opengl") => crate::renderer::skia::SkiaRendererAdapter::new_opengl,
            #[cfg(any(feature = "renderer-skia-opengl", feature = "renderer-skia-vulkan"))]
            Some("skia-software") => crate::renderer::skia::SkiaRendererAdapter::new_software,
            #[cfg(feature = "renderer-femtovg")]
            Some("femtovg") => crate::renderer::femtovg::FemtoVGRendererAdapter::new,
            #[cfg(feature = "renderer-software")]
            Some("software") => crate::renderer::sw::SoftwareRendererAdapter::new,
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
                return Err(format!("Timeout while waiting to activate session").into());
            }
        }

        Ok(Backend {
            #[cfg(feature = "libseat")]
            seat: Rc::new(RefCell::new(seat)),
            window: Default::default(),
            user_event_receiver: RefCell::new(Some(user_event_receiver)),
            proxy: Proxy::new(user_event_sender),
            renderer_factory,
            sel_clipboard: Default::default(),
            clipboard: Default::default(),
        })
    }
}

impl i_slint_core::platform::Platform for Backend {
    fn create_window_adapter(
        &self,
    ) -> Result<std::rc::Rc<dyn i_slint_core::window::WindowAdapter>, PlatformError> {
        #[cfg(feature = "libseat")]
        let device_accessor = |device: &std::path::Path| -> Result<Rc<OwnedFd>, PlatformError> {
            let device = self
                .seat
                .borrow_mut()
                .open_device(&device)
                .map_err(|e| format!("Error opening device: {e}"))?;

            // For polling for drm::control::Event::PageFlip we need a blocking FD. Would be better to do this non-blocking
            let fd = device.as_fd().as_raw_fd();
            let flags = nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_GETFL)
                .map_err(|e| format!("Error getting file descriptor flags: {e}"))?;
            let mut flags = nix::fcntl::OFlag::from_bits_retain(flags);
            flags.remove(nix::fcntl::OFlag::O_NONBLOCK);
            nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_SETFL(flags))
                .map_err(|e| format!("Error making device fd non-blocking: {e}"))?;

            // Safety: We take ownership of the now shared FD, ... although we should be using libseat's close_device....
            Ok(Rc::new(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) }))
        };

        #[cfg(not(feature = "libseat"))]
        let device_accessor = |device: &std::path::Path| -> Result<Rc<OwnedFd>, PlatformError> {
            let device = OpenOptions::new()
                .custom_flags((nix::fcntl::OFlag::O_NOCTTY | nix::fcntl::OFlag::O_CLOEXEC).bits())
                .read(true)
                .write(true)
                .open(device)
                .map(|file| file.into())
                .map_err(|e| format!("Error opening device: {e}"))?;

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

        let renderer = (self.renderer_factory)(&device_accessor)?;
        let adapter = FullscreenWindowAdapter::new(renderer, rotation)?;

        *self.window.borrow_mut() = Some(adapter.clone());

        Ok(adapter)
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        let mut event_loop: EventLoop<LoopData> =
            EventLoop::try_new().map_err(|e| format!("Error creating event loop: {}", e))?;

        let loop_signal = event_loop.get_signal();

        *self.proxy.loop_signal.lock().unwrap() = Some(loop_signal.clone());
        let quit_loop = self.proxy.quit_loop.clone();

        let mouse_position_property = input::LibInputHandler::init(
            &self.window,
            &event_loop.handle(),
            #[cfg(feature = "libseat")]
            &self.seat,
        )?;

        let Some(user_event_receiver) = self.user_event_receiver.borrow_mut().take() else {
            return Err(
                format!("Re-entering the linuxkms event loop is currently not supported").into()
            );
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

        while !quit_loop.load(std::sync::atomic::Ordering::Acquire) {
            i_slint_core::platform::update_timers_and_animations();

            // Only after updating the animation tick, invoke callbacks from invoke_from_event_loop(). They
            // might set animated properties, which requires an up-to-date start time.
            for callback in callbacks_to_invoke_per_iteration.take().into_iter() {
                callback();
            }

            if let Some(adapter) = self.window.borrow().as_ref() {
                adapter.clone().render_if_needed(mouse_position_property.as_ref())?;
            };

            let next_timeout = i_slint_core::platform::duration_until_next_timer_update();
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

pub type EventLoopHandle<'a> = calloop::LoopHandle<'a, LoopData>;
