// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::sync::OnceLock;
use std::time::Instant;

use slint_safeui_core::platform::Bgra8888Pixel;

const WIDTH_PIXELS: u32 = 640;
const HEIGHT_PIXELS: u32 = 480;
const PIXEL_STRIDE: u32 = WIDTH_PIXELS;

static SIM_THREAD: OnceLock<std::thread::Thread> = OnceLock::new();
static PIXEL_CHANNEL: OnceLock<smol::channel::Sender<Vec<Bgra8888Pixel>>> = OnceLock::new();

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
    let mut pixels = Vec::new();
    pixels.resize(PIXEL_STRIDE as usize * HEIGHT_PIXELS as usize, Bgra8888Pixel(0));
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

slint::slint! {import { AboutSlint, VerticalBox } from "std-widgets.slint";

export component MainWindow inherits Window {
    in property <image> image <=> screen.source;
    screen := Image { }
}
}

fn main() {
    let (pixel_sender, pixel_receiver) = smol::channel::unbounded();

    PIXEL_CHANNEL.set(pixel_sender).unwrap();

    let _thr = std::thread::spawn(|| {
        SIM_THREAD.set(std::thread::current()).unwrap();
        slint_safeui_core::slint_app_main()
    });

    let window = MainWindow::new().unwrap();

    let window_weak = window.as_weak();

    slint::spawn_local(async move {
        loop {
            if let Ok(source_pixels) = pixel_receiver.recv().await
                && let Some(window) = window_weak.upgrade()
            {
                let mut pixel_buf: slint::SharedPixelBuffer<slint::Rgb8Pixel> =
                    slint::SharedPixelBuffer::new(WIDTH_PIXELS, HEIGHT_PIXELS);
                let pixel_dest = pixel_buf.make_mut_slice();
                for i in 0..(WIDTH_PIXELS * HEIGHT_PIXELS) as usize {
                    let src = slint::platform::software_renderer::PremultipliedRgbaColor::from(
                        source_pixels[i],
                    );
                    pixel_dest[i] = slint::Rgb8Pixel { r: src.red, g: src.green, b: src.blue };
                }
                window.set_image(slint::Image::from_rgb8(pixel_buf));
            }
        }
    })
    .unwrap();

    window.run().unwrap();
}
