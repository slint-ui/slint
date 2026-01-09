// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::sync::OnceLock;
use std::time::Instant;

use slint_safeui_core::pixels::PlatformPixel;

pub const WIDTH_PIXELS: u32 = 640;
pub const HEIGHT_PIXELS: u32 = 480;
const PIXEL_STRIDE: u32 = WIDTH_PIXELS;

static SIM_THREAD: OnceLock<std::thread::Thread> = OnceLock::new();
static PIXEL_CHANNEL: OnceLock<smol::channel::Sender<Vec<PlatformPixel>>> = OnceLock::new();

pub fn init_channel(sender: smol::channel::Sender<Vec<PlatformPixel>>) {
    PIXEL_CHANNEL.set(sender).unwrap();
}

pub fn set_sim_thread(thread: std::thread::Thread) {
    SIM_THREAD.set(thread).unwrap();
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
    use slint::platform::software_renderer::TargetPixel;

    let mut pixels = Vec::new();
    pixels.resize(PIXEL_STRIDE as usize * HEIGHT_PIXELS as usize, PlatformPixel::background());
    let pixel_bytes: &mut [u8] = bytemuck::cast_slice_mut(&mut pixels);
    render_fn(
        user_data,
        pixel_bytes.as_mut_ptr() as *mut core::ffi::c_char,
        pixel_bytes.len() as u32,
        PIXEL_STRIDE,
    );

    PIXEL_CHANNEL.get().unwrap().send_blocking(pixels).unwrap();
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
        *width = WIDTH_PIXELS;
        *height = HEIGHT_PIXELS;
    }
}
