// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::sync::OnceLock;
use std::time::Instant;

use slint::platform::software_renderer::TargetPixel;
use slint_safeui_core::pixels::PlatformPixel;
use slint_safeui_core::{HEIGHT_PIXELS, SCALE_FACTOR, WIDTH_PIXELS};

pub const SCALED_WIDTH: u32 = (WIDTH_PIXELS as f32 * SCALE_FACTOR).round() as u32;
pub const SCALED_HEIGHT: u32 = (HEIGHT_PIXELS as f32 * SCALE_FACTOR).round() as u32;
const PIXEL_STRIDE: u32 = SCALED_WIDTH;

static SIM_THREAD: OnceLock<std::thread::Thread> = OnceLock::new();
static PIXEL_CHANNEL: OnceLock<smol::channel::Sender<Vec<slint::Rgb8Pixel>>> = OnceLock::new();

pub fn init_channel(sender: smol::channel::Sender<Vec<slint::Rgb8Pixel>>) {
    PIXEL_CHANNEL.set(sender).unwrap();
}

pub fn set_sim_thread(thread: std::thread::Thread) {
    SIM_THREAD.set(thread).unwrap();
}

fn convert_to_rgb8(pixels: &[PlatformPixel]) -> Vec<slint::Rgb8Pixel> {
    pixels
        .iter()
        .map(|&pixel| {
            #[cfg(feature = "pixel-bgra8888")]
            {
                let v = pixel.0;
                slint::Rgb8Pixel {
                    r: ((v >> 16) & 0xFF) as u8,
                    g: ((v >> 8) & 0xFF) as u8,
                    b: (v & 0xFF) as u8,
                }
            }

            #[cfg(feature = "pixel-rgb565")]
            {
                let r5 = ((pixel.0 >> 11) & 0x1F) as u8;
                let g6 = ((pixel.0 >> 5) & 0x3F) as u8;
                let b5 = (pixel.0 & 0x1F) as u8;

                slint::Rgb8Pixel {
                    r: (r5 << 3) | (r5 >> 2),
                    g: (g6 << 2) | (g6 >> 4),
                    b: (b5 << 3) | (b5 >> 2),
                }
            }

            #[cfg(feature = "pixel-rgb888")]
            {
                pixel
            }
        })
        .collect()
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
    let mut pixels =
        vec![PlatformPixel::background(); PIXEL_STRIDE as usize * SCALED_HEIGHT as usize];
    let pixel_bytes: &mut [u8] = bytemuck::cast_slice_mut(&mut pixels);
    render_fn(
        user_data,
        pixel_bytes.as_mut_ptr() as *mut core::ffi::c_char,
        pixel_bytes.len() as u32,
        PIXEL_STRIDE,
    );

    let display_pixels = convert_to_rgb8(&pixels);
    PIXEL_CHANNEL.get().unwrap().send_blocking(display_pixels).unwrap();
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
