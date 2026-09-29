// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The ESP32-S3-BOX-3 backend: an [`slint_safeui_app::Platform`] whose clock is
//! embassy's, whose input is the GT911 touch controller, and whose frame buffer
//! ends up on the ILI9342C panel.

use alloc::boxed::Box;
use core::time::Duration;

use embassy_futures::select::select;
use embedded_graphics_core::pixelcolor::Rgb565;
use esp_hal::gpio::{Flex, Output};
use slint_safeui_app::{AppEvent, Platform, slint_sc};

use crate::board::{BoardDisplay, BoardI2c, PANEL_HEIGHT, PANEL_WIDTH};

pub struct Esp32Platform {
    display: BoardDisplay,
    touch: gt911::Gt911Blocking<BoardI2c>,
    i2c: BoardI2c,
    /// The panel-sized RGB8 frame the scene renders into, in PSRAM.
    frame: Box<[u8]>,
    /// Where the ongoing touch last was, so that the release can be placed.
    pressed_at: Option<slint_sc::Point>,
    /// The GT911's interrupt line, which it pulls when a report is waiting.
    touch_int: Flex<'static>,
    // Kept alive for the lifetime of the backend.
    _backlight: Output<'static>,
}

impl Esp32Platform {
    pub fn new(
        display: BoardDisplay,
        touch: gt911::Gt911Blocking<BoardI2c>,
        i2c: BoardI2c,
        backlight: Output<'static>,
        touch_int: Flex<'static>,
    ) -> Self {
        let frame = alloc::vec![0u8; (PANEL_WIDTH * PANEL_HEIGHT * 3) as usize].into_boxed_slice();
        Self { display, touch, i2c, frame, pressed_at: None, touch_int, _backlight: backlight }
    }
}

impl Platform for Esp32Platform {
    fn now(&self) -> Duration {
        Duration::from_micros(embassy_time::Instant::now().as_micros())
    }

    fn size(&self) -> slint_sc::Size {
        slint_sc::Size::new(PANEL_WIDTH, PANEL_HEIGHT)
    }

    fn get_input_event(&mut self) -> Option<AppEvent> {
        // One read of the controller per call, so the caller's drain loop ends.
        match self.touch.get_touch(&mut self.i2c) {
            Ok(Some(point)) => {
                let position = slint_sc::Point::new(point.x as i32, point.y as i32);
                // A finger that stays down only updates where the release lands.
                self.pressed_at
                    .replace(position)
                    .is_none()
                    .then(|| AppEvent::Touch(slint_sc::TouchEvent::pressed(position)))
            }
            Ok(None) => self
                .pressed_at
                .take()
                .map(|position| AppEvent::Touch(slint_sc::TouchEvent::released(position))),
            Err(_) => None,
        }
    }

    async fn wait_for_more_events(&mut self, timeout: Option<Duration>) {
        // Any edge, because which one announces a report depends on how the
        // controller was configured. A missed one costs nothing: the next wake
        // reads the current state either way.
        let report = self.touch_int.wait_for_any_edge();
        match timeout {
            Some(timeout) => {
                let timeout = embassy_time::Duration::from_micros(timeout.as_micros() as u64);
                select(report, embassy_time::Timer::after(timeout)).await;
            }
            None => report.await,
        }
    }

    fn with_frame_buffer(&mut self, render: impl FnOnce(&mut [u8])) {
        let Self { display, frame, .. } = self;
        render(frame);

        // The scene renders RGB8, the panel takes RGB565.
        let pixels = frame
            .chunks_exact(3)
            .map(|pixel| Rgb565::new(pixel[0] >> 3, pixel[1] >> 2, pixel[2] >> 3));
        display
            .set_pixels(0, 0, (PANEL_WIDTH - 1) as u16, (PANEL_HEIGHT - 1) as u16, pixels)
            .expect("the panel accepts a full-screen update");
    }
}
