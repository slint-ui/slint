// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The safety-domain UI and its event loop, independent of any platform.
//!
//! A backend implements [`Platform`] — a clock, the display geometry, input
//! events, and a framebuffer — and drives the UI by calling [`app_main`]. The
//! desktop, C-FFI, and bare-metal examples each provide their own backend.

#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::Cell;
use core::time::Duration;

use slint::PhysicalSize;
use slint::platform::WindowEvent;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, TargetPixel};

slint::include_modules!();

/// An input event, or a request to leave [`app_main`].
pub enum AppEvent {
    /// An input event for the scene.
    Event(WindowEvent),
    /// Stop the event loop and return.
    Quit,
}

/// The interface a backend provides to [`app_main`].
pub trait Platform {
    /// The framebuffer's pixel format.
    type Pixel: TargetPixel;

    /// The time elapsed since the program started, driving timers and animations.
    fn now(&self) -> Duration;
    /// The physical size of the display, in pixels.
    fn size(&self) -> PhysicalSize;
    /// The scale factor: physical pixels per logical pixel.
    fn scale_factor(&self) -> f32 {
        1.
    }
    /// The next pending input event, if any.
    fn get_input_event(&mut self) -> Option<AppEvent>;
    /// Wait until an input event arrives or `timeout` elapses.
    // Every backend and the caller live in this workspace, so the missing auto
    // trait bounds the lint warns about never matter.
    #[allow(async_fn_in_trait)]
    async fn wait_for_more_events(&mut self, timeout: Option<Duration>);
    /// Render one frame into the framebuffer (its slice and pixel stride), then
    /// present it.
    fn with_frame_buffer(&mut self, render: impl FnOnce(&mut [Self::Pixel], usize));
}

/// A minimal Slint platform: it owns the software-rendered window and reports
/// the time [`app_main`] last sampled from the backend. Its event loop is never
/// run; [`app_main`] drives rendering directly.
struct SlintPlatform {
    window: Rc<MinimalSoftwareWindow>,
    clock: Rc<Cell<Duration>>,
}

impl slint::platform::Platform for SlintPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> Duration {
        self.clock.get()
    }
}

/// Drive the scene with the events and framebuffer `platform` provides, until it
/// reports [`AppEvent::Quit`].
pub async fn app_main(mut platform: impl Platform) -> Result<(), slint::PlatformError> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    let clock = Rc::new(Cell::new(platform.now()));
    slint::platform::set_platform(Box::new(SlintPlatform {
        window: window.clone(),
        clock: clock.clone(),
    }))
    .expect("platform already initialized");

    let ui = MainWindow::new()?;
    window.set_size(platform.size());
    window
        .dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: platform.scale_factor() });
    ui.show()?;

    loop {
        clock.set(platform.now());
        slint::platform::update_timers_and_animations();

        while let Some(event) = platform.get_input_event() {
            match event {
                AppEvent::Quit => {
                    ui.hide()?;
                    return Ok(());
                }
                AppEvent::Event(event) => window.dispatch_event(event),
            }
        }

        // Present a frame only when the window actually redraws, so a backend
        // that flushes whatever is in the buffer never shows a stale or empty one.
        window.draw_if_needed(|renderer| {
            platform.with_frame_buffer(|buffer, stride| {
                renderer.render(buffer, stride);
            });
        });

        let mut timeout = slint::platform::duration_until_next_timer_update();
        if window.has_active_animations() {
            let frame = Duration::from_millis(16);
            timeout = Some(timeout.map_or(frame, |t| t.min(frame)));
        }
        platform.wait_for_more_events(timeout).await;
    }
}

/// Run a future whose await points all resolve immediately — the case for a
/// backend whose [`Platform::wait_for_more_events`] blocks synchronously. A
/// single poll then runs [`app_main`] to completion, so no real waker or
/// executor is needed. A backend with genuinely pending waits (e.g. an
/// interrupt-driven embedded one) provides its own executor instead.
pub fn block_on<F: core::future::Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let waker = core::task::Waker::noop();
    let mut context = core::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        core::task::Poll::Ready(value) => value,
        core::task::Poll::Pending => panic!("the future parked, but this backend has no waker"),
    }
}
