// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Regression test for https://github.com/slint-ui/slint/issues/11765
//!
//! `sharedparley::text_size` borrows `font_context()` at the start, then calls
//! `text_item.text()` which evaluates a property binding. If that binding reads
//! a layout property of a container with other text elements (e.g.
//! `header.min-width / 1px`), computing that layout recurses into `text_size`
//! for those inner text elements, hitting a double `borrow_mut()` panic on the
//! shared `FontContext` `RefCell`.

mod common;

use slint::platform::software_renderer::{PremultipliedRgbaColor, SoftwareRenderer, TargetPixel};

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

const WIDTH: usize = 640;
const HEIGHT: usize = 480;

fn render(renderer: &SoftwareRenderer) {
    let mut buf = vec![TestPixel(false); WIDTH * HEIGHT];
    renderer.render(buf.as_mut_slice(), WIDTH);
}

#[test]
fn text_size_does_not_double_borrow_font_context() {
    let window = common::setup(WIDTH as u32, HEIGHT as u32);

    // Reproduces the pattern from issue #11765: a Text element whose `text`
    // property is bound to the min-width of a container that itself contains
    // text elements. When the layout system calls `text_size` for the outer
    // Text, evaluating the `text` binding forces layout of the inner container,
    // which calls `text_size` for the inner Text elements — all while the
    // outer `text_size` still holds `font_context().borrow_mut()`.
    slint::slint! {
        component Header {
            HorizontalLayout {
                Text {
                    text: "Hello World";
                    font-size: 16px;
                }
                Text {
                    text: "Some details";
                    font-size: 12px;
                }
            }
        }

        export component TestCase inherits Window {
            width: 640px;
            height: 480px;

            VerticalLayout {
                header := Header {}

                // This text's content depends on header's layout, creating
                // the re-entrant text_size call chain.
                Text {
                    text: header.min-width / 1px;
                }
            }
        }
    }

    let ui = TestCase::new().unwrap();
    ui.show().unwrap();

    // Trigger layout + render — this is where the double borrow would panic.
    window.draw_if_needed(render);
}
