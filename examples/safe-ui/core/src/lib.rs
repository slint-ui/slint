// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![no_std]

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

extern crate alloc;

pub mod pixels;
pub mod platform;

slint::include_modules!();

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

#[unsafe(no_mangle)]
pub extern "C" fn slint_app_main() {
    platform::slint_init_safeui_platform(WIDTH_PIXELS, HEIGHT_PIXELS, SCALE_FACTOR);

    let app = MainWindow::new().unwrap();

    app.show().unwrap();

    app.run().unwrap();
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
