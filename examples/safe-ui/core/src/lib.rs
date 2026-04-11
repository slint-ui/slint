// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![no_std]

// Enforce linking crate that provides critical section implementation.
// (We need these functions even though the lib code never calls them.)
#[cfg(feature = "cs-cortex-m")]
extern crate cortex_m;

// Enforce mutual exclusivity of pixel format
#[cfg(all(feature = "pixel-bgra8888", feature = "pixel-rgb565"))]
compile_error!("Cannot enable both pixel-bgra8888 and pixel-rgb565");

#[cfg(all(feature = "pixel-bgra8888", feature = "pixel-rgb888"))]
compile_error!("Cannot enable both pixel-bgra8888 and pixel-rgb888");

#[cfg(all(feature = "pixel-rgb565", feature = "pixel-rgb888"))]
compile_error!("Cannot enable both pixel-rgb565 and pixel-rgb888");

#[cfg(not(any(feature = "pixel-bgra8888", feature = "pixel-rgb565", feature = "pixel-rgb888")))]
compile_error!(
    "Must enable exactly one pixel format: pixel-bgra8888, pixel-rgb565 or pixel-rgb888"
);

pub mod pixels;
pub mod platform;

use slint_sc::Color;

slint_sc::include_modules!();

pub const WIDTH_PIXELS: u32 = match option_env!("SAFE_UI_WIDTH") {
    Some(s) => parse_u32(s),
    None => 320,
};

pub const HEIGHT_PIXELS: u32 = match option_env!("SAFE_UI_HEIGHT") {
    Some(s) => parse_u32(s),
    None => 240,
};

pub const SCALE_FACTOR: f32 = match option_env!("SAFE_UI_SCALE_FACTOR") {
    Some(s) => parse_f32(s),
    None => 2.0,
};

const COLOR_ROTATION_INTERVAL_MS: u32 = 1000;

const PALETTE: [Color; 3] = [
    Color::from_rgb_u8(0, 255, 0),
    Color::from_rgb_u8(255, 165, 0),
    Color::from_rgb_u8(255, 0, 0),
];

#[unsafe(no_mangle)]
pub extern "C" fn slint_app_main() {
    let window = MainWindow::new();

    let mut last_rotation_ms: u32 = 0;
    let mut palette_offset: usize = 0;
    apply_colors(&window, palette_offset);

    let mut request_redraw = true;

    loop {
        platform::drain_events();

        let now_ms = platform::duration_since_start_ms();
        if now_ms.wrapping_sub(last_rotation_ms) >= COLOR_ROTATION_INTERVAL_MS {
            last_rotation_ms = now_ms;
            palette_offset = (palette_offset + 1) % PALETTE.len();
            apply_colors(&window, palette_offset);
            request_redraw = true;
        }

        if request_redraw {
            request_redraw = false;
            platform::render_frame(|buffer| window.render(buffer));
        }

        let elapsed = now_ms.wrapping_sub(last_rotation_ms);
        let next_timeout_ms = COLOR_ROTATION_INTERVAL_MS.saturating_sub(elapsed);
        platform::wait_for_events_ms(next_timeout_ms as i32);
    }
}

fn apply_colors(window: &MainWindow, offset: usize) {
    window.set_color_0(PALETTE[offset % PALETTE.len()]);
    window.set_color_1(PALETTE[(offset + 1) % PALETTE.len()]);
    window.set_color_2(PALETTE[(offset + 2) % PALETTE.len()]);
}

const fn parse_u32(s: &str) -> u32 {
    let bytes = s.as_bytes();
    let mut result: u32 = 0;
    let mut i = 0;
    while i < bytes.len() {
        let digit = bytes[i];
        assert!(digit >= b'0' && digit <= b'9', "Invalid digit");
        result = result * 10 + (digit - b'0') as u32;
        i += 1;
    }
    result
}

const fn parse_f32(s: &str) -> f32 {
    let bytes = s.as_bytes();
    let mut integer: f64 = 0.0;
    let mut fraction: f64 = 0.0;
    let mut divisor: f64 = 1.0;
    let mut past_dot = false;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'.' {
            past_dot = true;
        } else {
            let digit = (b - b'0') as f64;
            if past_dot {
                divisor *= 10.0;
                fraction = fraction * 10.0 + digit;
            } else {
                integer = integer * 10.0 + digit;
            }
        }
        i += 1;
    }
    (integer + fraction / divisor) as f32
}
