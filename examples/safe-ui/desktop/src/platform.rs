// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The desktop backend: an [`slint_safeui_app::Platform`] whose display and
//! input are a Slint window running on the main thread. `app_main` runs on a
//! worker thread; this type bridges the two over channels.

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use slint_safeui_app::{AppEvent, Platform, slint_sc};

/// The size of the window, in pixels.
pub const WIDTH: u32 = 640;
pub const HEIGHT: u32 = 480;

/// A touch from the front-end. A plain, `Send` type crossing to the worker.
pub enum Input {
    Pressed { x: f32, y: f32 },
    Released { x: f32, y: f32 },
    Quit,
}

impl Input {
    fn into_event(self) -> AppEvent {
        let point = |x: f32, y: f32| slint_sc::Point::new(x as i32, y as i32);
        match self {
            Input::Pressed { x, y } => AppEvent::Touch(slint_sc::TouchEvent::pressed(point(x, y))),
            Input::Released { x, y } => {
                AppEvent::Touch(slint_sc::TouchEvent::released(point(x, y)))
            }
            Input::Quit => AppEvent::Quit,
        }
    }
}

/// Bridges `app_main` on the worker thread to the front-end window.
pub struct DesktopPlatform {
    pixels: smol::channel::Sender<Vec<u8>>,
    input: Receiver<Input>,
    start: Instant,
    /// Reused scratch the scene renders into before it is sent to the front-end.
    frame: Vec<u8>,
}

impl DesktopPlatform {
    pub fn new(pixels: smol::channel::Sender<Vec<u8>>, input: Receiver<Input>) -> Self {
        Self { pixels, input, start: Instant::now(), frame: Vec::new() }
    }
}

impl Platform for DesktopPlatform {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }

    fn size(&self) -> slint_sc::Size {
        slint_sc::Size::new(WIDTH, HEIGHT)
    }

    fn get_input_event(&mut self) -> Option<AppEvent> {
        self.input.try_recv().ok().map(Input::into_event)
    }

    async fn wait_for_more_events(&mut self, timeout: Option<Duration>) {
        // The front-end wakes this thread when it sends input.
        match timeout {
            Some(timeout) => std::thread::park_timeout(timeout),
            None => std::thread::park(),
        }
    }

    fn with_frame_buffer(&mut self, render: impl FnOnce(&mut [u8])) {
        self.frame.resize((WIDTH * HEIGHT * 3) as usize, 0);
        render(&mut self.frame);
        let _ = self.pixels.send_blocking(self.frame.clone());
    }
}
