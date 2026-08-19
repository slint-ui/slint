// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The safety-domain UI and its event loop, in the Slint SC subset.
//!
//! A backend implements [`Platform`] — a clock, the display size, touch events,
//! and an RGB8 frame buffer — and drives the UI by calling [`app_main`]. The
//! scene has no `Timer` and no model, so the color cycle the full-Slint version
//! expressed in `.slint` lives here in Rust.

#![no_std]

use core::time::Duration;

pub use slint_sc;

// The Slint SC scene (`MainWindow` and `MainWindowCallbacks`), generated from
// main.slint by build.rs running the slint-compiler with `--slint-sc`.
include!(concat!(env!("OUT_DIR"), "/main.rs"));

/// A touch of the display, or a request to leave [`app_main`].
pub enum AppEvent {
    /// A touch event for the scene.
    Touch(slint_sc::TouchEvent),
    /// Stop the event loop and return.
    Quit,
}

/// The interface a backend provides to [`app_main`].
pub trait Platform {
    /// The time elapsed since the program started, driving the color cycle.
    fn now(&self) -> Duration;
    /// The size of the display, in pixels.
    fn size(&self) -> slint_sc::Size;
    /// The next pending touch event, if any.
    fn get_input_event(&mut self) -> Option<AppEvent>;
    /// Wait until an input event arrives or `timeout` elapses.
    #[allow(async_fn_in_trait)]
    async fn wait_for_more_events(&mut self, timeout: Option<Duration>);
    /// Render one frame into the packed RGB8 buffer (`width * height * 3`
    /// bytes), then present it.
    fn with_frame_buffer(&mut self, render: impl FnOnce(&mut [u8]));
}

/// The colors the three telltales cycle through, one step per second.
const COLORS: [slint_sc::Color; 3] = [
    slint_sc::Color::from_rgb_u8(0x00, 0x80, 0x00), // green
    slint_sc::Color::from_rgb_u8(0xff, 0xa5, 0x00), // orange
    slint_sc::Color::from_rgb_u8(0xff, 0x00, 0x00), // red
];

/// One second between color steps.
const STEP_MS: u64 = 1000;

/// The scene declares no callbacks, so touch handling needs no state.
struct Callbacks;
impl MainWindowCallbacks for Callbacks {}

/// Drive the scene: cycle the telltale colors and deliver touch events the
/// `platform` reports, until it reports [`AppEvent::Quit`].
pub async fn app_main(mut platform: impl Platform) {
    let mut scene = MainWindow::new(platform.size());
    let mut callbacks = Callbacks;
    let mut current_step = usize::MAX;

    loop {
        let mut needs_redraw = false;

        // Derived from the clock, not a counter, so a late wake-up still lands right.
        let step = (platform.now().as_millis() as u64 / STEP_MS) as usize % COLORS.len();
        if step != current_step {
            current_step = step;
            scene.set_first_color(COLORS[step]);
            scene.set_second_color(COLORS[(step + 1) % COLORS.len()]);
            scene.set_third_color(COLORS[(step + 2) % COLORS.len()]);
            needs_redraw = true;
        }

        while let Some(event) = platform.get_input_event() {
            match event {
                AppEvent::Quit => return,
                AppEvent::Touch(touch) => {
                    scene.dispatch_touch_event(touch, &mut callbacks);
                    needs_redraw = true;
                }
            }
        }

        if needs_redraw {
            platform.with_frame_buffer(|buffer| {
                scene.render_rgb8(buffer).expect("the frame buffer matches the window size");
            });
        }

        // Sleep until the next step; an incoming event wakes us earlier.
        let elapsed = platform.now().as_millis() as u64;
        platform
            .wait_for_more_events(Some(Duration::from_millis(STEP_MS - elapsed % STEP_MS)))
            .await;
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
