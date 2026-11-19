// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The software renderer stores coordinates as `i16`. The cursor of a long text input, scrolled
//! more than 32767 physical pixels out of view, used to panic in the cast. With a bitmap font,
//! measuring and laying out such a text overflowed too.

use i_slint_core::graphics::{BitmapFont, BitmapGlyph, BitmapGlyphs};
use i_slint_core::slice::Slice;
use slint::Rgb8Pixel;
use slint::platform::WindowAdapter;
use slint::platform::software_renderer::{MinimalSoftwareWindow, TargetPixel};

mod common;

const WIDTH: usize = 100;
const HEIGHT: usize = 100;

slint::slint! {
    #[edition = "1.20"]
    export component TestCase inherits Window {
        width: 100px;
        height: 100px;
        in property <string> text;
        in property <int> end;
        in property <string> family;
        out property <length> text-height: ti.preferred-height;
        background: black;
        fl := Flickable {
            content-height: ti.height;
            ti := TextInput {
                text: root.text;
                width: parent.width;
                height: self.preferred-height;
                font-family: root.family;
                font-size: 20px;
                color: white;
            }
        }
        public function cursor-to-end() {
            ti.focus();
            ti.set-selection-offsets(root.end, root.end);
        }
        public function scroll-to-top() {
            fl.content-y = 0;
        }
    }
}

fn render<P: TargetPixel + Default>(window: &MinimalSoftwareWindow) -> Vec<P> {
    window.request_redraw();
    let mut buffer = vec![P::default(); WIDTH * HEIGHT];
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(&mut buffer, WIDTH);
    }));
    buffer
}

#[test]
fn far_away_cursor_does_not_panic() {
    let window = common::setup(WIDTH as u32, HEIGHT as u32);

    let ui = TestCase::new().unwrap();
    let text = "line\n".repeat(5000);
    ui.set_end(text.len() as i32);
    ui.set_text(text.into());
    ui.show().unwrap();

    render::<common::TestPixel>(&window);
    // Moving the cursor scrolls it into view; scroll back so it ends up far below the window.
    ui.invoke_cursor_to_end();
    render::<common::TestPixel>(&window);
    ui.invoke_scroll_to_top();
    render::<common::TestPixel>(&window);
}

/// A 2×2 square at the baseline, in a font with an ascent of 4 units and no descent.
/// As a signed distance field, it scales to the requested font size.
static TALL_FONT: BitmapFont = BitmapFont {
    family_name: Slice::from_slice(b"Tall"),
    character_map: Slice::from_slice(&['A']),
    units_per_em: 4.,
    ascent: 4.,
    descent: 0.,
    x_height: 2.,
    cap_height: 4.,
    glyphs: Slice::from_slice(&[BitmapGlyphs {
        pixel_size: 4,
        glyph_data: Slice::from_slice(&[
            BitmapGlyph {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_advance: 3 << 6,
                data: Slice::from_slice(&[]),
            },
            BitmapGlyph {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
                x_advance: 3 << 6,
                data: Slice::from_slice(&[255, 255, 255, 255]),
            },
        ]),
    }]),
    weight: 400,
    italic: false,
    sdf: true,
};

#[test]
fn bitmap_font_text_taller_than_i16_measures_and_renders() {
    // The `font-size`, as `TALL_FONT` has no leading.
    const LINE_HEIGHT: usize = 20;
    let window = common::setup(WIDTH as u32, HEIGHT as u32);
    window.renderer().register_bitmap_font(&TALL_FONT);

    let ui = TestCase::new().unwrap();
    ui.set_family("Tall".into());
    ui.show().unwrap();

    let render_lines = |lines: usize| {
        let text = vec!["A"; lines].join("\n");
        ui.set_end(text.len() as i32);
        ui.set_text(text.into());
        assert_eq!(ui.get_text_height(), (lines * LINE_HEIGHT) as f32, "{lines} lines");
        ui.invoke_cursor_to_end();
        render::<Rgb8Pixel>(&window)
    };

    let short = render_lines(10);
    assert!(short.iter().any(|pixel| *pixel != Rgb8Pixel::default()));
    for lines in [2_000, 40_000] {
        assert!(render_lines(lines) == short, "{lines} lines");
    }
}
