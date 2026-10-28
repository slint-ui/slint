// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Drawing, measuring or hit-testing a text evaluates a binding that measures another text (#14554).

mod common;

use common::TestPixel;
use i_slint_core::graphics::{BitmapFont, BitmapGlyph, BitmapGlyphs};
use i_slint_core::slice::Slice;
use slint::LogicalPosition;
use slint::platform::software_renderer::MinimalSoftwareWindow;
use slint::platform::{WindowAdapter, WindowEvent};

const WIDTH: usize = 100;
const HEIGHT: usize = 50;

fn render(window: &MinimalSoftwareWindow) {
    let mut buffer = vec![TestPixel::default(); WIDTH * HEIGHT];
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(&mut buffer, WIDTH);
    }));
}

static GLYPHS: [BitmapGlyphs; 1] = [BitmapGlyphs {
    pixel_size: 4,
    glyph_data: Slice::from_slice(&[BitmapGlyph {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
        x_advance: 3 << 6,
        data: Slice::from_slice(&[255; 4]),
    }]),
}];

static BITMAP_FONT: BitmapFont = BitmapFont {
    family_name: Slice::from_slice(b"Bitmap"),
    character_map: Slice::from_slice(&['A']),
    units_per_em: 4.,
    ascent: 4.,
    descent: 0.,
    x_height: 2.,
    cap_height: 4.,
    glyphs: Slice::from_slice(&GLYPHS),
    weight: 400,
    italic: false,
    sdf: false,
};

#[test]
fn draw_text() {
    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            reference := Text { width: 10px; height: 10px; text: "reference"; }
            Text {
                y: 20px;
                width: 100px;
                height: 20px;
                text: "" + reference.preferred-width / 1px;
            }
        }
    }
    let window = common::setup(WIDTH as u32, HEIGHT as u32);
    let app = App::new().unwrap();
    app.show().unwrap();
    render(&window);
}

#[test]
fn measure_word_wrapped_text() {
    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            reference := Text { text: "reference"; }
            measured := Text {
                text: "measured";
                wrap: word-wrap;
                max-lines: reference.preferred-width > 0px ? 1 : 2;
            }
            out property <length> measured-width: measured.preferred-width;
        }
    }
    let _window = common::setup(WIDTH as u32, HEIGHT as u32);
    let app = App::new().unwrap();
    assert!(app.get_measured_width() > 0.);
}

#[test]
fn draw_styled_text_link() {
    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            reference := Text { width: 10px; height: 10px; text: "reference"; }
            StyledText {
                y: 20px;
                width: 100px;
                height: 20px;
                text: @markdown("[link](https://slint.dev)");
                link-color: reference.preferred-width > 0px ? red : blue;
            }
        }
    }
    let window = common::setup(WIDTH as u32, HEIGHT as u32);
    let app = App::new().unwrap();
    app.show().unwrap();
    render(&window);
}

#[test]
fn draw_styled_text_selection() {
    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            reference := Text { width: 10px; height: 10px; text: "reference"; }
            StyledText {
                y: 20px;
                width: 100px;
                height: 20px;
                text: @markdown("selected");
                selection-background-color: reference.preferred-width > 0px ? red : blue;
                init => {
                    self.select-all();
                }
            }
        }
    }
    let window = common::setup(WIDTH as u32, HEIGHT as u32);
    let app = App::new().unwrap();
    app.show().unwrap();
    render(&window);
}

#[test]
fn draw_text_input_selection() {
    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            reference := Text { width: 10px; height: 10px; text: "reference"; }
            TextInput {
                y: 20px;
                width: 100px;
                height: 20px;
                text: "input";
                selection-background-color: reference.preferred-width > 0px ? red : blue;
            }
        }
    }
    let window = common::setup(WIDTH as u32, HEIGHT as u32);
    let app = App::new().unwrap();
    app.show().unwrap();
    render(&window);
}

#[test]
fn hover_styled_text() {
    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            reference := Text { width: 10px; height: 10px; text: "reference"; }
            StyledText {
                y: 20px;
                width: 100px;
                height: 20px;
                text: @markdown("[link](https://slint.dev)");
                link-color: reference.preferred-width > 0px ? red : blue;
            }
        }
    }
    let _window = common::setup(WIDTH as u32, HEIGHT as u32);
    let app = App::new().unwrap();
    app.show().unwrap();
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(5., 25.) });
}

#[test]
fn draw_bitmap_font_text() {
    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            reference := Text { width: 10px; height: 10px; text: "reference"; }
            Text {
                y: 20px;
                width: 100px;
                height: 20px;
                text: "A";
                font-family: "Bitmap";
                font-size: 4px;
                line-height-factor: reference.preferred-height / 10px;
            }
        }
    }
    let window = common::setup(WIDTH as u32, HEIGHT as u32);
    window.renderer().register_bitmap_font(&BITMAP_FONT);
    let app = App::new().unwrap();
    app.show().unwrap();
    render(&window);
}

#[test]
fn draw_bitmap_font_text_input() {
    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            in property <string> reference-text: "reference";
            reference := Text { width: 10px; height: 10px; text: root.reference-text; }
            TextInput {
                y: 20px;
                width: 100px;
                height: 20px;
                text: "A";
                font-family: "Bitmap";
                font-size: 4px;
                color: reference.preferred-width > 0px ? red : blue;
            }
        }
    }
    let window = common::setup(WIDTH as u32, HEIGHT as u32);
    window.renderer().register_bitmap_font(&BITMAP_FONT);
    let app = App::new().unwrap();
    app.show().unwrap();
    // The `TextInput` reads its color when it's created, so dirty the binding again.
    app.set_reference_text("changed".into());
    render(&window);
}
