// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The content widths of a word-wrapping text are shaped once and cached, rather than
//! re-shaped on every horizontal layout pass.

mod common;

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, SoftwareRenderer, TargetPixel,
};
use std::rc::Rc;

#[allow(dead_code)]
#[derive(Clone, Copy, Default)]
struct TestPixel(bool);

impl TargetPixel for TestPixel {
    fn blend(&mut self, _color: PremultipliedRgbaColor) {
        *self = Self(true);
    }
    fn from_rgb(_red: u8, _green: u8, _blue: u8) -> Self {
        Self(true)
    }
}

const WIDTH: usize = 200;
const HEIGHT: usize = 100;

/// Renders a frame and returns how many times the content widths had to be shaped so far.
fn render_and_misses(window: &Rc<MinimalSoftwareWindow>) -> u64 {
    let mut count = 0;
    window.request_redraw();
    // The buffer follows the window: a scale factor change resizes it.
    let size = window.size();
    let (width, height) = (size.width as usize, size.height as usize);
    window.draw_if_needed(|renderer: &SoftwareRenderer| {
        let mut buf = vec![TestPixel(false); width * height];
        renderer.render(buf.as_mut_slice(), width);
        count = renderer.text_layout_cache().content_widths_miss_count();
    });
    count
}

slint::slint! {
    export component TestComponent inherits Window {
        in property <string> label: "Hello wrapping world";
        in property <length> size: 16px;
        in property <int> limit: 0;
        // Reading a layout property pulls the text's horizontal layout info, which is
        // what asks for the content widths.
        out property <length> min-w: t.min-width;
        t := Text {
            text: label;
            font-size: size;
            max-lines: limit;
            overflow: clip;
            wrap: word-wrap;
        }
    }
}

fn setup() -> (Rc<MinimalSoftwareWindow>, TestComponent) {
    let window = common::setup(WIDTH as u32, HEIGHT as u32);
    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();
    (window, ui)
}

#[test]
fn repeated_layout_passes_shape_once() {
    let (window, ui) = setup();

    let width = ui.get_min_w();
    assert!(width > 0.);
    for _ in 0..4 {
        assert_eq!(render_and_misses(&window), 1, "the content widths were shaped again");
    }
    assert_eq!(ui.get_min_w(), width);
}

#[test]
fn text_change_invalidates() {
    let (window, ui) = setup();
    let width = ui.get_min_w();
    assert_eq!(render_and_misses(&window), 1);

    ui.set_label("Supercalifragilistic".into());

    assert_eq!(render_and_misses(&window), 2);
    assert_ne!(ui.get_min_w(), width, "the widths didn't follow the text");
}

#[test]
fn font_size_change_invalidates() {
    let (window, ui) = setup();
    let width = ui.get_min_w();
    assert_eq!(render_and_misses(&window), 1);

    ui.set_size(32.);

    assert_eq!(render_and_misses(&window), 2);
    assert!(ui.get_min_w() > width, "the widths didn't follow the font size");
}

#[test]
fn scale_factor_change_invalidates() {
    let (window, ui) = setup();
    let width = ui.get_min_w();
    assert_eq!(render_and_misses(&window), 1);

    window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 2. });
    // A real windowing system resizes the surface along with the scale factor.
    window.set_size(slint::PhysicalSize::new(WIDTH as u32 * 2, HEIGHT as u32 * 2));

    // Shaping is redone because glyph advances are in physical pixels. The reported
    // width is logical, so the value itself is unchanged.
    assert_eq!(render_and_misses(&window), 2);
    assert_eq!(ui.get_min_w(), width);
}

#[test]
fn line_limit_change_invalidates() {
    let (window, ui) = setup();
    ui.set_label("short\nSupercalifragilistic".into());
    ui.set_limit(1);
    let width = ui.get_min_w();
    let misses = render_and_misses(&window);

    ui.set_limit(2);

    assert_eq!(render_and_misses(&window), misses + 1);
    assert_ne!(ui.get_min_w(), width, "the widths didn't follow the line limit");
}
