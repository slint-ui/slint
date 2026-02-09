// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod desktop_platform;

use desktop_platform::{SCALED_HEIGHT, SCALED_WIDTH};
use minifb::{Key, Window, WindowOptions};

fn main() {
    let (pixel_sender, pixel_receiver) = smol::channel::unbounded();

    desktop_platform::init_channel(pixel_sender);

    let _sim_thread = std::thread::spawn(|| {
        desktop_platform::set_sim_thread(std::thread::current());
        slint_safeui_core::slint_app_main()
    });

    let mut window = Window::new(
        "Slint SafeUI Simulator",
        SCALED_WIDTH as usize,
        SCALED_HEIGHT as usize,
        WindowOptions::default(),
    )
    .unwrap_or_else(|e| {
        panic!("{}", e);
    });

    window.set_target_fps(60);

    let mut buffer: Vec<u32> = vec![0; (SCALED_WIDTH * SCALED_HEIGHT) as usize];

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Perform drain loop dropping all previous frames
        // only keeping latest frame.
        let mut new_frame = None;
        while let Ok(frame) = pixel_receiver.try_recv() {
            new_frame = Some(frame);
        }

        if let Some(pixels) = new_frame {
            for (i, pixel) in pixels.iter().enumerate() {
                if i < buffer.len() {
                    buffer[i] =
                        ((pixel.r as u32) << 16) | ((pixel.g as u32) << 8) | (pixel.b as u32);
                }
            }
        }

        window.update_with_buffer(&buffer, SCALED_WIDTH as usize, SCALED_HEIGHT as usize).unwrap();
    }
}
