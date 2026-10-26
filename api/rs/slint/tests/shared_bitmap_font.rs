// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use i_slint_core::graphics::{BitmapFont, BitmapGlyph, BitmapGlyphs, OwnedStorage};
use i_slint_core::slice::Slice;
use slint::platform::software_renderer::{LineBufferProvider, MinimalSoftwareWindow};
use slint::platform::{PlatformError, WindowAdapter};
use slint::{ComponentHandle, Rgb8Pixel};

const WIDTH: usize = 8;
const HEIGHT: usize = 8;

slint::slint! {
    #[slint_edition = "1.19"]
    export component App inherits Window {
        in property <string> family;
        background: black;
        Text {
            x: 0;
            y: 0;
            text: "AA";
            font-family: family;
            font-size: 4px;
            color: white;
        }
    }
}

struct TestPlatform(Rc<MinimalSoftwareWindow>);

impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}

/// A 2×2 square, with an advance of 3 pixels
const GLYPH_DATA: [u8; 4] = [255, 128, 128, 255];

static GLYPHS: [BitmapGlyphs; 1] = [BitmapGlyphs {
    pixel_size: 4,
    glyph_data: Slice::from_slice(&[BitmapGlyph {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
        x_advance: 3 << 6,
        data: Slice::from_slice(&GLYPH_DATA),
    }]),
}];

const fn static_font(family_name: &'static [u8], sdf: bool) -> BitmapFont {
    BitmapFont {
        family_name: Slice::from_slice(family_name),
        character_map: Slice::from_slice(&['A']),
        units_per_em: 4.,
        ascent: 4.,
        descent: 0.,
        x_height: 2.,
        cap_height: 4.,
        glyphs: Slice::from_slice(&GLYPHS),
        weight: 400,
        italic: false,
        sdf,
    }
}

static STATIC_FONT: BitmapFont = static_font(b"Static", false);
static STATIC_SDF_FONT: BitmapFont = static_font(b"StaticSdf", true);

fn shared_font(family_name: &str, glyph_data: [u8; 4], sdf: bool) -> Rc<BitmapFont<OwnedStorage>> {
    Rc::new(BitmapFont::<OwnedStorage> {
        family_name: family_name.as_bytes().to_vec(),
        character_map: vec!['A'],
        units_per_em: 4.,
        ascent: 4.,
        descent: 0.,
        x_height: 2.,
        cap_height: 4.,
        glyphs: vec![BitmapGlyphs::<OwnedStorage> {
            pixel_size: 4,
            glyph_data: vec![BitmapGlyph::<OwnedStorage> {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
                x_advance: 3 << 6,
                data: glyph_data.to_vec(),
            }],
        }],
        weight: 400,
        italic: false,
        sdf,
    })
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

fn render(
    window: &MinimalSoftwareWindow,
    app: &App,
    family: &str,
    by_line: bool,
) -> Vec<Rgb8Pixel> {
    app.set_family(family.into());
    window.request_redraw();
    let mut buffer = vec![Rgb8Pixel::default(); WIDTH * HEIGHT];
    assert!(window.draw_if_needed(|renderer| {
        if by_line {
            renderer.render_by_line(Lines(&mut buffer));
        } else {
            renderer.render(&mut buffer, WIDTH);
        }
    }));
    buffer
}

#[test]
fn a_shared_bitmap_font_renders_like_a_static_one() {
    let window = MinimalSoftwareWindow::new(Default::default());
    slint::platform::set_platform(Box::new(TestPlatform(window.clone()))).unwrap();
    window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    let app = App::new().unwrap();
    app.show().unwrap();

    let same = shared_font("Shared", GLYPH_DATA, false);
    let same_sdf = shared_font("SharedSdf", GLYPH_DATA, true);
    let other = shared_font("Other", [255; 4], false);
    window.renderer().register_bitmap_font(&STATIC_FONT);
    window.renderer().register_bitmap_font(&STATIC_SDF_FONT);
    window.renderer().register_shared_bitmap_font(&same);
    window.renderer().register_shared_bitmap_font(&same_sdf);
    window.renderer().register_shared_bitmap_font(&other);

    for (static_family, shared_family) in [("Static", "Shared"), ("StaticSdf", "SharedSdf")] {
        let with_static_font = render(&window, &app, static_family, false);
        assert!(with_static_font.iter().any(|pixel| *pixel != Rgb8Pixel::default()));
        for by_line in [false, true] {
            for family in [static_family, shared_family] {
                assert_eq!(
                    render(&window, &app, family, by_line),
                    with_static_font,
                    "{family}, by line: {by_line}"
                );
            }
        }
    }
    let with_static_font = render(&window, &app, "Static", false);
    // Not the fallback to the static font
    let with_other_font = render(&window, &app, "Other", false);
    assert_ne!(with_other_font, with_static_font);

    // Without its owner, the font is gone
    drop(other);
    assert_ne!(render(&window, &app, "Other", false), with_other_font);
}
