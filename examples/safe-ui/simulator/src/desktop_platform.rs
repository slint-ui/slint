// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::sync::OnceLock;
use std::time::Instant;

use bytemuck::Zeroable;
use slint_safeui_core::pixels::{PlatformPixel, Rgb8Pixel};
use slint_safeui_core::{HEIGHT_PIXELS, SCALE_FACTOR, WIDTH_PIXELS};

pub const SCALED_WIDTH: u32 = (WIDTH_PIXELS as f32 * SCALE_FACTOR).round() as u32;
pub const SCALED_HEIGHT: u32 = (HEIGHT_PIXELS as f32 * SCALE_FACTOR).round() as u32;
const PIXEL_STRIDE: u32 = SCALED_WIDTH;

static SIM_THREAD: OnceLock<std::thread::Thread> = OnceLock::new();
static PIXEL_CHANNEL: OnceLock<smol::channel::Sender<Vec<Rgb8Pixel>>> = OnceLock::new();

pub fn init_channel(sender: smol::channel::Sender<Vec<Rgb8Pixel>>) {
    PIXEL_CHANNEL.set(sender).unwrap();
}

pub fn set_sim_thread(thread: std::thread::Thread) {
    SIM_THREAD.set(thread).unwrap();
}

fn convert_to_rgb8(pixels: &[PlatformPixel]) -> Vec<Rgb8Pixel> {
    pixels.iter().map(|&p| Rgb8Pixel::from(p)).collect()
}

#[unsafe(no_mangle)]
extern "C" fn slint_safeui_platform_wait_for_events(max_wait_milliseconds: i32) {
    if max_wait_milliseconds > 0 {
        std::thread::park_timeout(std::time::Duration::from_millis(max_wait_milliseconds as u64))
    } else {
        std::thread::park();
    }
}

#[unsafe(no_mangle)]
extern "C" fn slint_safeui_platform_wake() {
    if let Some(thread) = SIM_THREAD.get() {
        thread.unpark();
    }
}

#[unsafe(no_mangle)]
extern "C" fn slint_safeui_platform_render(
    user_data: *mut (),
    render_fn: extern "C" fn(
        *mut (),
        *mut core::ffi::c_char,
        buffer_size_bytes: u32,
        pixel_stride: u32,
    ),
) {
    // Since we don't have the trait TargetPixel::background(), just use zeroed memory
    // assuming that color black is a valid background for all pixel formats.
    let mut pixels = vec![PlatformPixel::zeroed(); PIXEL_STRIDE as usize * SCALED_HEIGHT as usize];

    let pixel_bytes_ptr = pixels.as_mut_ptr() as *mut core::ffi::c_char;
    let byte_size = (pixels.len() * std::mem::size_of::<PlatformPixel>()) as u32;

    render_fn(user_data, pixel_bytes_ptr, byte_size, PIXEL_STRIDE);

    let display_pixels = convert_to_rgb8(&pixels);
    if let Some(chan) = PIXEL_CHANNEL.get() {
        chan.send_blocking(display_pixels).unwrap();
    }
}

#[unsafe(no_mangle)]
extern "C" fn slint_safeui_platform_duration_since_start() -> i32 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);

    start.elapsed().as_millis() as i32
}

#[unsafe(no_mangle)]
extern "C" fn slint_safeui_platform_get_screen_size(width: *mut u32, height: *mut u32) {
    unsafe {
        *width = SCALED_WIDTH;
        *height = SCALED_HEIGHT;
    }
}
