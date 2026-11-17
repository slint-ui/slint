// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The software renderer stores coordinates as `i16`. A `Clip` with a border radius used to panic
//! in the cast when it was more than 32767 physical pixels out of view, or taller than that.
//! Whatever is in view has to render as it does for a clip within that range.

mod common;

use slint::platform::software_renderer::{
    LineBufferProvider, MinimalSoftwareWindow, RepaintBufferType,
};
use slint::{ComponentHandle, Rgb8Pixel};

const WIDTH: usize = 40;
const HEIGHT: usize = 30;

const BORDER: Rgb8Pixel = Rgb8Pixel { r: 0xff, g: 0xff, b: 0xff };
const CHILD: Rgb8Pixel = Rgb8Pixel { r: 0xff, g: 0, b: 0 };

slint::slint! {
    #[edition = "1.20"]
    export component TestCase inherits Window {
        in property <length> clip-y;
        in property <length> clip-height;
        in property <length> scroll;
        background: #000000;
        Flickable {
            content-height: 100000px;
            content-y: -root.scroll;
            Rectangle {
                x: 4px;
                y: root.clip-y;
                width: 32px;
                height: root.clip-height;
                border-radius: 10px;
                border-width: 2px;
                border-color: #ffffff;
                background: #3c78c8;
                clip: true;
                Rectangle { width: 50%; background: #ff0000; }
            }
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

/// Renders the frame line by line and into a buffer, which have to agree.
fn render(window: &MinimalSoftwareWindow) -> Vec<Rgb8Pixel> {
    let mut by_line = vec![Rgb8Pixel::default(); WIDTH * HEIGHT];
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        renderer.render_by_line(Lines(&mut by_line));
    }));
    let mut buffer = vec![Rgb8Pixel::default(); WIDTH * HEIGHT];
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        renderer.render(&mut buffer, WIDTH);
    }));
    assert_same(&by_line, &buffer, "line by line and into a buffer");
    buffer
}

fn assert_same(a: &[Rgb8Pixel], b: &[Rgb8Pixel], what: &str) {
    if let Some(i) = a.iter().zip(b).position(|(a, b)| a != b) {
        panic!("{what}: pixel ({}, {}) differs: {:?} vs {:?}", i % WIDTH, i / WIDTH, a[i], b[i]);
    }
}

#[test]
fn far_away_rounded_clip_draws_nothing() {
    let window = common::setup(WIDTH as u32, HEIGHT as u32);
    let ui = TestCase::new().unwrap();
    ui.set_clip_height(20.);
    ui.show().unwrap();

    ui.set_clip_y(200.);
    let near = render(&window);
    assert!(near.iter().all(|p| *p == Rgb8Pixel::default()));
    ui.set_clip_y(40_000.);
    assert_same(&render(&window), &near, "40000px below");
    ui.set_clip_y(-40_000.);
    assert_same(&render(&window), &near, "40000px above");
}

#[test]
fn tall_rounded_clip_draws_like_a_short_one() {
    let window = common::setup(WIDTH as u32, HEIGHT as u32);
    let ui = TestCase::new().unwrap();
    ui.set_clip_y(4.);
    ui.show().unwrap();

    // Each view shows the clip's top, its bottom, or neither.
    // In the middle, both ends of the 70000px clip lie past the `i16` range.
    let views: [(&str, fn(f32) -> f32); 3] = [
        ("top", |_| 0.),
        ("bottom", |height| 4. + height - HEIGHT as f32 + 4.),
        ("middle", |height| height / 2.),
    ];
    for (view, scroll) in views {
        ui.set_clip_height(100.);
        ui.set_scroll(scroll(100.));
        let short = render(&window);
        assert!(short.contains(&BORDER) && short.contains(&CHILD), "{view}: nothing drawn");
        ui.set_clip_height(70_000.);
        ui.set_scroll(scroll(70_000.));
        assert_same(&render(&window), &short, view);
    }
}
