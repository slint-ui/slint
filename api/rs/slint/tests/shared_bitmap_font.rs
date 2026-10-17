// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use i_slint_core::graphics::{
    BitmapFont, BitmapGlyph, BitmapGlyphs, SharedBitmapFont, SharedBitmapGlyph, SharedBitmapGlyphs,
};
use i_slint_core::slice::Slice;
use slint::platform::software_renderer::MinimalSoftwareWindow;
use slint::platform::{PlatformError, WindowAdapter};
use slint::{ComponentHandle, Rgb8Pixel};

const WIDTH: usize = 8;
const HEIGHT: usize = 8;

slint::slint! {
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

static STATIC_FONT: BitmapFont = BitmapFont {
    family_name: Slice::from_slice(b"Static"),
    character_map: Slice::from_slice(&['A']),
    units_per_em: 4.,
    ascent: 4.,
    descent: 0.,
    x_height: 2.,
    cap_height: 4.,
    glyphs: Slice::from_slice(&[BitmapGlyphs {
        pixel_size: 4,
        glyph_data: Slice::from_slice(&[BitmapGlyph {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            x_advance: 3 << 6,
            data: Slice::from_slice(&GLYPH_DATA),
        }]),
    }]),
    weight: 400,
    italic: false,
    sdf: false,
};

fn shared_font(family_name: &str, glyph_data: [u8; 4]) -> Rc<SharedBitmapFont> {
    Rc::new(SharedBitmapFont {
        family_name: family_name.into(),
        character_map: vec!['A'],
        units_per_em: 4.,
        ascent: 4.,
        descent: 0.,
        x_height: 2.,
        cap_height: 4.,
        glyphs: vec![SharedBitmapGlyphs {
            pixel_size: 4,
            glyph_data: vec![SharedBitmapGlyph {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
                x_advance: 3 << 6,
                data: glyph_data.into(),
            }],
        }],
        weight: 400,
        italic: false,
        sdf: false,
    })
}

fn render(window: &MinimalSoftwareWindow, app: &App, family: &str) -> Vec<Rgb8Pixel> {
    app.set_family(family.into());
    let mut buffer = vec![Rgb8Pixel::default(); WIDTH * HEIGHT];
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(&mut buffer, WIDTH);
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

    let same = shared_font("Shared", GLYPH_DATA);
    let other = shared_font("Other", [255; 4]);
    window.renderer().register_bitmap_font(&STATIC_FONT);
    window.renderer().register_shared_bitmap_font(&same);
    window.renderer().register_shared_bitmap_font(&other);

    let with_static_font = render(&window, &app, "Static");
    assert!(with_static_font.iter().any(|pixel| *pixel != Rgb8Pixel::default()));
    assert_eq!(render(&window, &app, "Shared"), with_static_font);
    // Not the fallback to the static font
    let with_other_font = render(&window, &app, "Other");
    assert_ne!(with_other_font, with_static_font);

    // Without its owner, the font is gone
    drop(other);
    window.request_redraw();
    assert_ne!(render(&window, &app, "Other"), with_other_font);
}
