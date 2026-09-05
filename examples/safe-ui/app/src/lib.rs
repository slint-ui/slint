// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The safety-domain UI and its event loop, in the Slint SC subset.
//!
//! A backend implements [`Platform`] — a clock, the display size, touch events,
//! and an RGB8 frame buffer — and drives the UI by calling [`app_main`]. The
//! scene has no `Timer` and no model, so the airlock sequence that full Slint
//! would express in `.slint` lives here in Rust.

#![no_std]

use core::time::Duration;

pub use slint_sc;

// The Slint SC scene (`MainWindow`, `MainWindowCallbacks` and `TransitView`),
// generated from main.slint by build.rs running the slint-compiler with
// `--slint-sc`.
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
    /// The time elapsed since the program started, driving the airlock sequence.
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

/// How long the doors take to secure.
const SECURING: Duration = Duration::from_secs(6);
/// How long the chamber takes to equalize its pressure.
const EQUALIZING: Duration = Duration::from_secs(9);
/// How often a running phase updates the countdown and the progress ring.
const TICK: Duration = Duration::from_millis(100);

/// The airlock sequence the screen shows: the state, and the phase timing
/// behind it.
struct Airlock {
    view: TransitView,
    /// When the running phase started, on the platform clock.
    phase_start: Duration,
    /// The platform clock of the current pass, so a handler that starts a
    /// phase knows when it began.
    now: Duration,
    seconds_remaining: i32,
    progress_percent: i32,
}

impl Airlock {
    fn new() -> Self {
        Self {
            view: TransitView::Ready,
            phase_start: Duration::ZERO,
            now: Duration::ZERO,
            seconds_remaining: 0,
            progress_percent: 0,
        }
    }

    /// Back to an idle airlock, ready to admit the next occupant.
    fn reset(&mut self) {
        self.view = TransitView::Ready;
        self.seconds_remaining = 0;
        self.progress_percent = 0;
    }

    /// Whether a phase is running, and the screen therefore has to keep up
    /// with the clock.
    fn is_running(&self) -> bool {
        matches!(self.view, TransitView::Securing | TransitView::Equalizing)
    }

    /// Carry the running phase to `now`, ending it once its time is up.
    ///
    /// The phases are derived from elapsed time rather than counted, so a late
    /// wake-up still lands right.
    fn advance(&mut self, now: Duration) {
        self.now = now;

        if self.view == TransitView::Securing {
            let elapsed = now.saturating_sub(self.phase_start);
            if elapsed >= SECURING {
                self.phase_start += SECURING;
                self.view = TransitView::Equalizing;
            } else {
                self.seconds_remaining = whole_seconds(SECURING - elapsed);
            }
        }

        if self.view == TransitView::Equalizing {
            let elapsed = now.saturating_sub(self.phase_start);
            self.seconds_remaining = whole_seconds(EQUALIZING.saturating_sub(elapsed));
            self.progress_percent = percent_of(elapsed, EQUALIZING);
            if elapsed >= EQUALIZING {
                self.view = TransitView::ReadyToOpen;
            }
        }
    }

    /// Copy the state into the scene, and report whether that changed anything.
    fn apply(&self, scene: &mut MainWindow) -> bool {
        let mut changed = false;
        if scene.get_view() != self.view {
            scene.set_view(self.view);
            changed = true;
        }
        if scene.get_seconds_remaining() != self.seconds_remaining {
            scene.set_seconds_remaining(self.seconds_remaining);
            changed = true;
        }
        if scene.get_progress_percent() != self.progress_percent {
            scene.set_progress_percent(self.progress_percent);
            changed = true;
        }
        changed
    }
}

/// The scene reaches a request only from the view it belongs to, since the
/// other `TouchArea`s sit off-screen, and each handler checks it again so the
/// sequence holds whatever the screen delivers.
impl MainWindowCallbacks for Airlock {
    fn on_enter_requested(&mut self, _scene: &mut MainWindow) {
        if self.view == TransitView::Ready {
            self.phase_start = self.now;
            self.seconds_remaining = whole_seconds(SECURING);
            self.progress_percent = 0;
            self.view = TransitView::Securing;
        }
    }

    fn on_exit_outer_requested(&mut self, _scene: &mut MainWindow) {
        if self.view == TransitView::ReadyToOpen {
            self.view = TransitView::Complete;
        }
    }

    fn on_exit_inner_requested(&mut self, _scene: &mut MainWindow) {
        if self.view == TransitView::Complete {
            self.reset();
        }
    }

    fn on_fault_requested(&mut self, _scene: &mut MainWindow) {
        self.view = TransitView::Fault;
    }

    fn on_reset_requested(&mut self, _scene: &mut MainWindow) {
        self.reset();
    }
}

/// The whole seconds `remaining` covers, rounded up, so the countdown shows
/// zero only once the phase is over.
fn whole_seconds(remaining: Duration) -> i32 {
    (remaining.as_millis() as u64).div_ceil(1000) as i32
}

/// How far `elapsed` has come through `total`, in percent, capped at 100.
fn percent_of(elapsed: Duration, total: Duration) -> i32 {
    let percent = elapsed.as_millis() * 100 / total.as_millis();
    percent.min(100) as i32
}

/// Drive the scene: run the airlock sequence off the platform clock and
/// deliver the touch events the `platform` reports, until it reports
/// [`AppEvent::Quit`].
pub async fn app_main(mut platform: impl Platform) {
    let mut scene = MainWindow::new(platform.size());
    let mut airlock = Airlock::new();
    // Nothing has reached the display yet, so the first pass renders.
    let mut needs_redraw = true;

    loop {
        airlock.advance(platform.now());
        needs_redraw |= airlock.apply(&mut scene);

        while let Some(event) = platform.get_input_event() {
            match event {
                AppEvent::Quit => return,
                AppEvent::Touch(touch) => {
                    scene.dispatch_touch_event(touch, &mut airlock);
                    needs_redraw |= airlock.apply(&mut scene);
                }
            }
        }

        if needs_redraw {
            platform.with_frame_buffer(|buffer| {
                scene.render_rgb8(buffer).expect("the frame buffer matches the window size");
            });
            needs_redraw = false;
        }

        // Sleep until the next tick of the running phase, or until a touch
        // arrives when the airlock is resting.
        let timeout = airlock.is_running().then(|| {
            let into_tick = platform.now().as_millis() as u64 % TICK.as_millis() as u64;
            TICK - Duration::from_millis(into_tick)
        });
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
