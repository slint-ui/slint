// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The desktop backend: an [`slint_safeui_app::Platform`] whose display and
//! input are a Slint window running on the main thread. `app_main` runs on a
//! worker thread; this type bridges the two over channels.

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use slint::LogicalPosition;
use slint::PhysicalSize;
use slint::SharedString;
use slint::platform::{PointerEventButton, WindowEvent};
use slint_safeui_app::{AppEvent, Platform};

/// The size of the window, in pixels. The scene renders at scale one, so these
/// are both logical and physical.
pub const WIDTH: u32 = 640;
pub const HEIGHT: u32 = 480;

/// Input from the front-end. A plain, `Send` type so it crosses to the worker
/// thread without any Slint reference-counted value.
pub enum Input {
    PointerPressed { x: f32, y: f32, button: PointerEventButton },
    PointerReleased { x: f32, y: f32, button: PointerEventButton },
    PointerMoved { x: f32, y: f32 },
    PointerScrolled { x: f32, y: f32, delta_x: f32, delta_y: f32 },
    KeyPressed(char),
    KeyRepeated(char),
    KeyReleased(char),
    Quit,
}

impl Input {
    /// The event to deliver to the scene.
    fn into_event(self) -> AppEvent {
        let position = |x: f32, y: f32| LogicalPosition::new(x, y);
        let text = |c: char| SharedString::from(c.encode_utf8(&mut [0u8; 4]) as &str);
        AppEvent::Event(match self {
            Input::PointerPressed { x, y, button } => {
                WindowEvent::PointerPressed { position: position(x, y), button }
            }
            Input::PointerReleased { x, y, button } => {
                WindowEvent::PointerReleased { position: position(x, y), button }
            }
            Input::PointerMoved { x, y } => WindowEvent::PointerMoved { position: position(x, y) },
            Input::PointerScrolled { x, y, delta_x, delta_y } => {
                WindowEvent::PointerScrolled { position: position(x, y), delta_x, delta_y }
            }
            Input::KeyPressed(c) => WindowEvent::KeyPressed { text: text(c) },
            Input::KeyRepeated(c) => WindowEvent::KeyPressRepeated { text: text(c) },
            Input::KeyReleased(c) => WindowEvent::KeyReleased { text: text(c) },
            Input::Quit => return AppEvent::Quit,
        })
    }
}

/// Bridges `app_main` on the worker thread to the front-end window.
pub struct DesktopPlatform {
    pixels: smol::channel::Sender<Vec<slint::Rgb8Pixel>>,
    input: Receiver<Input>,
    start: Instant,
    /// Reused scratch the scene renders into before it is sent to the front-end.
    frame: Vec<slint::Rgb8Pixel>,
}

impl DesktopPlatform {
    pub fn new(
        pixels: smol::channel::Sender<Vec<slint::Rgb8Pixel>>,
        input: Receiver<Input>,
    ) -> Self {
        Self { pixels, input, start: Instant::now(), frame: Vec::new() }
    }
}

impl Platform for DesktopPlatform {
    type Pixel = slint::Rgb8Pixel;

    fn now(&self) -> Duration {
        self.start.elapsed()
    }

    fn size(&self) -> PhysicalSize {
        PhysicalSize::new(WIDTH, HEIGHT)
    }

    fn get_input_event(&mut self) -> Option<AppEvent> {
        self.input.try_recv().ok().map(Input::into_event)
    }

    async fn wait_for_more_events(&mut self, timeout: Option<Duration>) {
        // The front-end unparks this thread when it sends input.
        match timeout {
            Some(timeout) => std::thread::park_timeout(timeout),
            None => std::thread::park(),
        }
    }

    fn with_frame_buffer(&mut self, render: impl FnOnce(&mut [Self::Pixel], usize)) {
        self.frame.resize((WIDTH * HEIGHT) as usize, slint::Rgb8Pixel { r: 0, g: 0, b: 0 });
        render(&mut self.frame, WIDTH as usize);
        let _ = self.pixels.send_blocking(self.frame.clone());
    }
}
