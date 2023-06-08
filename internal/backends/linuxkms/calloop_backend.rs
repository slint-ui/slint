// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use calloop::EventLoop;
use i_slint_core::platform::PlatformError;
use i_slint_core::platform::WindowAdapter;

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
    window: RefCell<Option<Rc<FullscreenWindowAdapter>>>,
    user_event_receiver: RefCell<Option<calloop::channel::Channel<Box<dyn FnOnce() + Send>>>>,
    proxy: Proxy,
    renderer_factory: fn() -> Result<
        Box<dyn crate::fullscreenwindowadapter::Renderer>,
        i_slint_core::platform::PlatformError,
    >,
}

impl Backend {
    pub fn new() -> Self {
        Self::new_with_renderer_by_name(None)
    }
    pub fn new_with_renderer_by_name(renderer_name: Option<&str>) -> Self {
        let (user_event_sender, user_event_receiver) = calloop::channel::channel();

        let renderer_factory = match renderer_name {
            #[cfg(feature = "renderer-skia-vulkan")]
            Some("skia-vulkan") => crate::renderer::skia::SkiaRendererAdapter::new_vulkan,
            #[cfg(feature = "renderer-skia-opengl")]
            Some("skia-opengl") => crate::renderer::skia::SkiaRendererAdapter::new_opengl,
            #[cfg(feature = "renderer-femtovg")]
            Some("femtovg") => crate::renderer::femtovg::FemtoVGRendererAdapter::new,
            None => crate::renderer::try_skia_then_femtovg,
            Some(renderer_name) => {
                eprintln!(
                    "slint linuxkms backend: unrecognized renderer {}, falling back default",
                    renderer_name
                );
                crate::renderer::try_skia_then_femtovg
            }
        };

        Backend {
            window: Default::default(),
            user_event_receiver: RefCell::new(Some(user_event_receiver)),
            proxy: Proxy::new(user_event_sender),
            renderer_factory,
        }
    }
}

impl i_slint_core::platform::Platform for Backend {
    fn create_window_adapter(
        &self,
    ) -> Result<
        std::rc::Rc<dyn i_slint_core::window::WindowAdapter>,
        i_slint_core::platform::PlatformError,
    > {
        let renderer = (self.renderer_factory)()?;
        let adapter = FullscreenWindowAdapter::new(renderer)?;

        *self.window.borrow_mut() = Some(adapter.clone());

        Ok(adapter)
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        let adapter = self.window.borrow().as_ref().unwrap().clone();

        let mut event_loop: EventLoop<LoopData> =
            EventLoop::try_new().map_err(|e| format!("Error creating event loop: {}", e))?;

        let loop_signal = event_loop.get_signal();

        *self.proxy.loop_signal.lock().unwrap() = Some(loop_signal.clone());
        let quit_loop = self.proxy.quit_loop.clone();

        #[cfg(not(any(
            target_family = "windows",
            target_os = "macos",
            target_os = "ios",
            target_arch = "wasm32"
        )))]
        input::LibInputHandler::init(adapter.window(), &event_loop.handle())?;

        let Some(user_event_receiver) = self.user_event_receiver.borrow_mut().take() else {
            return Err(
                format!("Re-entering the linuxkms event loop is currently not supported").into()
            );
        };

        event_loop
            .handle()
            .insert_source(user_event_receiver, |event, _, _| {
                let calloop::channel::Event::Msg(callback) = event else { return };
                callback();
            })
            .map_err(
                |e: calloop::InsertError<calloop::channel::Channel<Box<dyn FnOnce() + Send>>>| {
                    format!("Error registering user event channel source: {e}")
                },
            )?;

        // Render first frame
        adapter.render_if_needed()?;

        let mut loop_data = LoopData::default();

        quit_loop.store(false, std::sync::atomic::Ordering::Release);

        while !quit_loop.load(std::sync::atomic::Ordering::Acquire) {
            i_slint_core::platform::update_timers_and_animations();

            let next_timeout = if adapter.window().has_active_animations() {
                Some(std::time::Duration::from_millis(16))
            } else {
                i_slint_core::platform::duration_until_next_timer_update()
            };

            event_loop
                .dispatch(next_timeout, &mut loop_data)
                .map_err(|e| format!("Error dispatch events: {e}"))?;

            adapter.render_if_needed()?;
        }

        Ok(())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
        Some(Box::new(self.proxy.clone()))
    }
}

#[derive(Default)]
struct LoopData {}
