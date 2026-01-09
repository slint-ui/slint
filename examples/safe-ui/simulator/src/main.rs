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

    let window = MainWindow::new().unwrap();

    let window_weak = window.as_weak();

    slint::spawn_local(async move {
        use desktop_platform::{HEIGHT_PIXELS, WIDTH_PIXELS};

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
