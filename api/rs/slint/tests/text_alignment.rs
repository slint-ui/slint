// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Regression test for text horizontal alignment with the software renderer.
//! Alignment (center/right) was broken by the parley 0.9 upgrade: the new API removed the
//! width parameter from `align()` and relies on `break_all_lines()` instead, but the call
//! to `break_all_lines` was filtered to pass `None` for no-wrap text, leaving no container
//! width for alignment to work against.

mod common;

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, TargetPixel,
};
use std::rc::Rc;

const WIDTH: u32 = 300;
const HEIGHT: u32 = 30;

fn setup() -> Rc<MinimalSoftwareWindow> {
    common::setup(WIDTH, HEIGHT)
}

/// A pixel that blends RGBA colors so we can inspect the resulting color.
#[derive(Clone, Copy)]
struct RgbPixel {
    r: u8,
    g: u8,
    b: u8,
}

impl Default for RgbPixel {
    fn default() -> Self {
        RgbPixel { r: 0, g: 0, b: 0 }
    }
}

impl TargetPixel for RgbPixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let inv_alpha = 255u32 - color.alpha as u32;
        self.r = (color.red as u32 + self.r as u32 * inv_alpha / 255).min(255) as u8;
        self.g = (color.green as u32 + self.g as u32 * inv_alpha / 255).min(255) as u8;
        self.b = (color.blue as u32 + self.b as u32 * inv_alpha / 255).min(255) as u8;
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        RgbPixel { r, g, b }
    }
}

fn render(window: &Rc<MinimalSoftwareWindow>) -> Vec<RgbPixel> {
    let mut buf = vec![RgbPixel::default(); (WIDTH * HEIGHT) as usize];
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(buf.as_mut_slice(), WIDTH as usize);
    });
    buf
}

/// Returns the leftmost column that contains a pixel significantly darker than white.
/// Text rendered in black on a white background shows up as dark pixels.
fn leftmost_glyph_col(buf: &[RgbPixel]) -> Option<u32> {
    let width = WIDTH as usize;
    let height = HEIGHT as usize;
    for col in 0..width {
        for row in 0..height {
            let p = buf[row * width + col];
            if p.r < 200 || p.g < 200 || p.b < 200 {
                return Some(col as u32);
            }
        }
    }
    None
}

/// Returns the rightmost column that contains a dark pixel.
fn rightmost_glyph_col(buf: &[RgbPixel]) -> Option<u32> {
    let width = WIDTH as usize;
    let height = HEIGHT as usize;
    for col in (0..width).rev() {
        for row in 0..height {
            let p = buf[row * width + col];
            if p.r < 200 || p.g < 200 || p.b < 200 {
                return Some(col as u32);
            }
        }
    }
    None
}

/// Regression test: horizontal-alignment must position text correctly.
///
/// Before the fix, center and right alignment produced the same pixel layout as
/// left alignment because `break_all_lines` was called with `None` for no-wrap
/// text, giving parley no container width to align against.
#[test]
fn text_horizontal_alignment() {
    let window = setup();

    slint::slint! {
        export component TestCase inherits Window {
            in property <int> align: 0;
            background: white;
            Text {
                width: 100%;
                height: 100%;
                text: "XXXX";
                color: black;
                font-size: 14px;
                horizontal-alignment: align == 1 ? TextHorizontalAlignment.center
                                    : align == 2 ? TextHorizontalAlignment.right
                                                 : TextHorizontalAlignment.left;
            }
        }
    }

    let ui = TestCase::new().unwrap();
    ui.show().unwrap();

    // --- Left alignment ---
    ui.set_align(0);
    let left_buf = render(&window);
    let left_start = leftmost_glyph_col(&left_buf).expect("left-aligned text must be rendered");
    let left_end = rightmost_glyph_col(&left_buf).expect("left-aligned text must be rendered");
    let text_width = left_end - left_start + 1;

    assert!(
        left_start < WIDTH / 4,
        "Left-aligned text should start near the left edge, but started at column {left_start}"
    );

    // --- Right alignment ---
    ui.set_align(2);
    let right_buf = render(&window);
    let right_start = leftmost_glyph_col(&right_buf).expect("right-aligned text must be rendered");
    let right_end = rightmost_glyph_col(&right_buf).expect("right-aligned text must be rendered");

    assert!(
        right_end >= WIDTH - WIDTH / 4,
        "Right-aligned text should end near the right edge, but ended at column {right_end}"
    );
    assert!(
        right_start > left_start + text_width,
        "Right-aligned text (starts at {right_start}) must start well to the right of \
         left-aligned text (starts at {left_start}, width {text_width})"
    );
    assert_ne!(
        left_buf.iter().map(|p| [p.r, p.g, p.b]).collect::<Vec<_>>(),
        right_buf.iter().map(|p| [p.r, p.g, p.b]).collect::<Vec<_>>(),
        "Left and right alignment produced identical renders — alignment is broken"
    );

    // --- Center alignment ---
    ui.set_align(1);
    let center_buf = render(&window);
    let center_start =
        leftmost_glyph_col(&center_buf).expect("center-aligned text must be rendered");

    assert!(
        center_start > left_start,
        "Center-aligned text (starts at {center_start}) must start to the right of \
         left-aligned text (starts at {left_start})"
    );
    assert!(
        center_start < right_start,
        "Center-aligned text (starts at {center_start}) must start to the left of \
         right-aligned text (starts at {right_start})"
    );
}
