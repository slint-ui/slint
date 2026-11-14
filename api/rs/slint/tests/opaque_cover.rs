// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! An opaque image that covers a line lets the renderer skip the background and everything behind
//! it. Whatever it leaves unpainted would then show the previous buffer content.
//! The screenshot tests cover the formats a `.slint` file can load.
//! An opaque rounded rectangle covers the lines that cross no corner; the corner lines are drawn
//! as before, and the result must also match the buffer renderer.

mod common;

use slint::platform::software_renderer::{
    LineBufferProvider, MinimalSoftwareWindow, RepaintBufferType,
};
use slint::{ComponentHandle, Image, Rgb8Pixel, SharedPixelBuffer};

const WIDTH: usize = 37;
const HEIGHT: usize = 23;

slint::slint! {
    #[edition = "1.20"]
    export component Scene inherits Window {
        in property <image> picture;
        background: #ffffff;

        Rectangle { width: 20px; height: 20px; background: #ff0000; }
        Image { source: root.picture; width: 100%; height: 100%; }
    }
}

slint::slint! {
    #[edition = "1.20"]
    export component RoundedScene inherits Window {
        in property <length> border-width;
        in property <color> fill: #3c78c8;
        background: #ffffff;

        Rectangle { x: 3px; y: 2px; width: 20px; height: 15px; background: #ff0000; }
        Rectangle {
            width: 100%; height: 100%;
            border-radius: 6px;
            background: root.fill;
            border-width: root.border-width;
            border-color: #102040;
        }
    }
}

struct Lines<'a>(&'a mut [Rgb8Pixel]);

impl LineBufferProvider for Lines<'_> {
    type TargetPixel = Rgb8Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Rgb8Pixel]),
    ) {
        render_fn(&mut self.0[line * WIDTH..][range]);
    }
}

fn render(window: &MinimalSoftwareWindow, fill: Rgb8Pixel) -> Vec<Rgb8Pixel> {
    let mut buffer = vec![fill; WIDTH * HEIGHT];
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        renderer.render_by_line(Lines(&mut buffer));
    }));
    buffer
}

fn render_to_buffer(window: &MinimalSoftwareWindow, fill: Rgb8Pixel) -> Vec<Rgb8Pixel> {
    let mut buffer = vec![fill; WIDTH * HEIGHT];
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        renderer.render(&mut buffer, WIDTH);
    }));
    buffer
}

fn first_difference(a: &[Rgb8Pixel], b: &[Rgb8Pixel]) -> Option<(usize, usize)> {
    a.iter().zip(b).position(|(a, b)| a != b).map(|i| (i % WIDTH, i / WIDTH))
}

fn pixels<T: Clone + Default>(pixel: impl Fn(usize, usize) -> T) -> SharedPixelBuffer<T> {
    let mut buffer = SharedPixelBuffer::new(WIDTH as u32, HEIGHT as u32);
    for (i, p) in buffer.make_mut_slice().iter_mut().enumerate() {
        *p = pixel(i % WIDTH, i / WIDTH);
    }
    buffer
}

#[test]
fn covering_image_paints_every_pixel() {
    let window = common::setup(WIDTH as u32, HEIGHT as u32);

    let scene = Scene::new().unwrap();
    scene.show().unwrap();

    let pictures = [
        (
            "RGB8",
            Image::from_rgb8(pixels(|x, y| Rgb8Pixel { r: x as u8 * 6, g: y as u8 * 10, b: 99 })),
        ),
        #[cfg(feature = "image-pixel-format-rgb565")]
        ("RGB565", Image::from_rgb565(pixels(|x, y| slint::Rgb565Pixel((x * 1500 + y) as u16)))),
        #[cfg(feature = "image-pixel-format-gray8")]
        ("Gray8", Image::from_gray8(pixels(|x, y| slint::Gray8Pixel((x * 6 + y) as u8)))),
    ];

    for (format, picture) in pictures {
        scene.set_picture(picture);
        let a = render(&window, Rgb8Pixel { r: 1, g: 2, b: 3 });
        let b = render(&window, Rgb8Pixel { r: 250, g: 251, b: 252 });
        if let Some(i) = a.iter().zip(&b).position(|(a, b)| a != b) {
            panic!(
                "{format}: pixel ({}, {}) depends on the previous buffer content",
                i % WIDTH,
                i / WIDTH
            );
        }
    }
}

#[test]
fn covering_rounded_rectangle_paints_every_pixel() {
    let window = common::setup(WIDTH as u32, HEIGHT as u32);

    let scene = RoundedScene::new().unwrap();
    scene.show().unwrap();

    for (case, border_width, fill) in [
        ("fill only", 0.0, slint::Color::from_rgb_u8(0x3c, 0x78, 0xc8)),
        ("with border", 2.0, slint::Color::from_rgb_u8(0x3c, 0x78, 0xc8)),
        ("translucent fill", 0.0, slint::Color::from_argb_u8(0x80, 0x3c, 0x78, 0xc8)),
    ] {
        scene.set_border_width(border_width);
        scene.set_fill(fill);
        let a = render(&window, Rgb8Pixel { r: 1, g: 2, b: 3 });
        let b = render(&window, Rgb8Pixel { r: 250, g: 251, b: 252 });
        if let Some((x, y)) = first_difference(&a, &b) {
            panic!("{case}: pixel ({x}, {y}) depends on the previous buffer content");
        }
        let c = render_to_buffer(&window, Rgb8Pixel { r: 1, g: 2, b: 3 });
        if let Some((x, y)) = first_difference(&a, &c) {
            panic!("{case}: pixel ({x}, {y}) differs between render_by_line and render");
        }
    }
}
