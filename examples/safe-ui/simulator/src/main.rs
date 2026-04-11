// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod desktop_platform;

slint::slint! {import { AboutSlint, VerticalBox } from "std-widgets.slint";

export component MainWindow inherits Window {
    in property <image> image <=> screen.source;
    screen := Image { }
}
}

fn main() {
    let (pixel_sender, pixel_receiver) = smol::channel::unbounded();

    desktop_platform::init_channel(pixel_sender);

    let _sim_thread = std::thread::spawn(|| {
        desktop_platform::set_sim_thread(std::thread::current());
        slint_safeui_core::slint_app_main()
    });

    let platform = i_slint_backend_selector::create_backend().unwrap();
    let ctx = i_slint_backend_selector::SlintContext::new(platform);
    let window = MainWindow::new_with_context(ctx.clone()).unwrap();
    let window_weak = window.as_weak();

    ctx.spawn_local(async move {
        use desktop_platform::{SCALED_HEIGHT, SCALED_WIDTH};

        loop {
            if let Ok(source_pixels) = pixel_receiver.recv().await
                && let Some(window) = window_weak.upgrade()
            {
                let mut pixel_buf: slint::SharedPixelBuffer<slint::Rgb8Pixel> =
                    slint::SharedPixelBuffer::new(SCALED_WIDTH, SCALED_HEIGHT);
                let pixel_dest = pixel_buf.make_mut_slice();
                pixel_dest.copy_from_slice(&source_pixels);

                window.set_image(slint::Image::from_rgb8(pixel_buf));
            }
        }
    })
    .unwrap();

    window.run().unwrap();
}

// The tests live inline in `main.rs` because they reference the
// `#[no_mangle]` `slint_safeui_platform_*` symbols defined in
// `desktop_platform`, which cannot be compiled twice (as they would be if
// the tests sat in `tests/`).
#[cfg(test)]
mod tests {
    use std::time::Duration;

    /// Headless end-to-end test for the safe-ui core's colour rotation:
    /// spawns `slint_app_main` and asserts that the first rectangle's
    /// pixels differ across three frames sampled ~1s apart.
    #[test]
    fn colors_rotate_over_time() {
        let (tx, rx) = smol::channel::unbounded();
        crate::desktop_platform::init_channel(tx);

        std::thread::spawn(|| {
            crate::desktop_platform::set_sim_thread(std::thread::current());
            slint_safeui_core::slint_app_main()
        });

        let mut frames = Vec::new();
        while frames.len() < 3 {
            let frame = smol::block_on(async {
                smol::future::or(
                    async { rx.recv().await.ok() },
                    async {
                        smol::Timer::after(Duration::from_secs(5)).await;
                        None
                    },
                )
                .await
            })
            .expect("timed out waiting for a frame");
            frames.push(frame);
        }

        let w = crate::desktop_platform::SCALED_WIDTH as usize;
        // Centre of the first rectangle (20 logical px × 2 scale).
        let sample = |frame: &[slint::Rgb8Pixel]| frame[20 * w + 20];

        let s0 = sample(&frames[0]);
        let s1 = sample(&frames[1]);
        let s2 = sample(&frames[2]);
        assert_ne!(
            (s0.r, s0.g, s0.b),
            (s1.r, s1.g, s1.b),
            "rectangle colour did not change after ~1s"
        );
        assert_ne!(
            (s1.r, s1.g, s1.b),
            (s2.r, s2.g, s2.b),
            "rectangle colour did not change after ~2s"
        );
    }
}
