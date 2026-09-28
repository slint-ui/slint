// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Draws an image of each of the optional pixel formats with the software renderer, which is
//! what those formats are for: handing the buffer to Slint without expanding it first.

use std::rc::Rc;

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{PlatformError, WindowAdapter};
use slint::{Rgb8Pixel, SharedPixelBuffer};

const SIZE: usize = 4;

struct TestPlatform(Rc<MinimalSoftwareWindow>);

impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}

slint::slint! {
    export component TestCase inherits Window {
        in property <image> source;
        Image {
            x: 0; y: 0;
            width: 4phx;
            height: 4phx;
            source: root.source;
            image-rendering: pixelated;
        }
    }
}

fn render(window: &MinimalSoftwareWindow) -> Vec<Rgb8Pixel> {
    let mut buffer = vec![Rgb8Pixel::new(0, 0, 0); SIZE * SIZE];
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(&mut buffer, SIZE);
    }));
    buffer
}

#[test]
fn optional_pixel_formats_render() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(window.clone()))).unwrap();
    let ui = TestCase::new().unwrap();
    window.set_size(slint::PhysicalSize::new(SIZE as u32, SIZE as u32));
    ui.show().unwrap();

    // Gray8: the luminance is written to all three channels.
    let mut gray = SharedPixelBuffer::<slint::Gray8Pixel>::new(SIZE as u32, SIZE as u32);
    for (i, p) in gray.make_mut_slice().iter_mut().enumerate() {
        *p = slint::Gray8Pixel::new(if i % 2 == 0 { 0x00 } else { 0xff });
    }
    ui.set_source(slint::Image::from_gray8(gray));
    let rendered = render(&window);
    for (i, p) in rendered.iter().enumerate() {
        let v = if i % 2 == 0 { 0x00 } else { 0xff };
        assert_eq!(*p, Rgb8Pixel::new(v, v, v), "gray8 mismatch at {i}");
    }

    // RGB565: a saturated component reaches 0xff on an RGB8 target.
    let mut rgb565 = SharedPixelBuffer::<slint::Rgb565Pixel>::new(SIZE as u32, SIZE as u32);
    for (i, p) in rgb565.make_mut_slice().iter_mut().enumerate() {
        *p = match i % 3 {
            0 => slint::Rgb565Pixel::from_rgb(0xff, 0, 0),
            1 => slint::Rgb565Pixel::from_rgb(0, 0xff, 0),
            _ => slint::Rgb565Pixel::from_rgb(0, 0, 0xff),
        };
    }
    ui.set_source(slint::Image::from_rgb565(rgb565));
    let rendered = render(&window);

    for (i, p) in rendered.iter().enumerate() {
        let expected = match i % 3 {
            0 => Rgb8Pixel::new(0xff, 0, 0),
            1 => Rgb8Pixel::new(0, 0xff, 0),
            _ => Rgb8Pixel::new(0, 0, 0xff),
        };
        assert_eq!(*p, expected, "rgb565 mismatch at {i}");
    }
}
