// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! An opaque rounded rectangle covers the lines that cross no corner, so the line renderer skips
//! the background and everything behind it there. The corner lines are drawn as before. Either way
//! the result must not depend on the previous buffer content and must match the buffer renderer.

mod common;

use slint::platform::software_renderer::{
    LineBufferProvider, MinimalSoftwareWindow, RepaintBufferType,
};
use slint::{ComponentHandle, Rgb8Pixel};

const WIDTH: usize = 37;
const HEIGHT: usize = 23;

slint::slint! {
    #[edition = "1.20"]
    export component Scene inherits Window {
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

fn render_by_line(window: &MinimalSoftwareWindow, fill: Rgb8Pixel) -> Vec<Rgb8Pixel> {
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

#[test]
fn covering_rounded_rectangle_paints_every_pixel() {
    let window = common::setup(WIDTH as u32, HEIGHT as u32);

    let scene = Scene::new().unwrap();
    scene.show().unwrap();

    for (case, border_width, fill) in [
        ("fill only", 0.0, slint::Color::from_rgb_u8(0x3c, 0x78, 0xc8)),
        ("with border", 2.0, slint::Color::from_rgb_u8(0x3c, 0x78, 0xc8)),
        ("translucent fill", 0.0, slint::Color::from_argb_u8(0x80, 0x3c, 0x78, 0xc8)),
    ] {
        scene.set_border_width(border_width);
        scene.set_fill(fill);
        let a = render_by_line(&window, Rgb8Pixel { r: 1, g: 2, b: 3 });
        let b = render_by_line(&window, Rgb8Pixel { r: 250, g: 251, b: 252 });
        if let Some((x, y)) = first_difference(&a, &b) {
            panic!("{case}: pixel ({x}, {y}) depends on the previous buffer content");
        }
        let c = render_to_buffer(&window, Rgb8Pixel { r: 1, g: 2, b: 3 });
        if let Some((x, y)) = first_difference(&a, &c) {
            panic!("{case}: pixel ({x}, {y}) differs between render_by_line and render");
        }
    }
}
