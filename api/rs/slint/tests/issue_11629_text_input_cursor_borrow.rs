// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Regression test for https://github.com/slint-ui/slint/issues/11629
//!
//! `SoftwareRenderer::text_input_cursor_rect_for_byte_offset` used to hold a
//! `borrow_mut()` on the slint context's font_context across a call to
//! `text_input.width()`. When the layout had been dirtied (e.g. because the
//! text just changed), evaluating that property recurses into the renderer's
//! `text_size`, which tries to borrow the same `RefCell` again and panics.

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

const WIDTH: usize = 200;
const HEIGHT: usize = 32;

fn render(renderer: &SoftwareRenderer) {
    let mut buf = vec![TestPixel(false); WIDTH * HEIGHT];
    renderer.render(buf.as_mut_slice(), WIDTH);
}

#[test]
fn text_input_cursor_rect_does_not_recurse_into_font_context() {
    // Force the buggy code path. The default (VectorFont, parley enabled) branch
    // correctly drops the borrow before recursing; the (PixelFont, _) and
    // (VectorFont, parley disabled) branches share the same hold-across-width()
    // bug. The env var is the simplest way to drive the latter from a test that
    // doesn't ship its own bitmap font. See `parley_disabled()` in the software
    // renderer.
    //
    // SAFETY: this is the only test in this binary, so no other thread reads
    // env vars concurrently.
    unsafe {
        std::env::set_var("SLINT_SOFTWARE_RENDERER_PARLEY_DISABLED", "1");
    }

    let window = common::setup(WIDTH as u32, HEIGHT as u32);

    slint::slint! {
        export component TestCase inherits Window {
            width: 200px;
            height: 32px;
            forward-focus: ti;
            HorizontalLayout {
                ti := TextInput {
                    text: "Hello";
                    font-size: 12px;
                }
                // This element's layout-info depends on `ti.text`, so changing
                // the text dirties the parent layout's solution and therefore
                // `ti.width()`. Re-evaluating that binding from inside the
                // renderer is what triggers the recursive borrow.
                Text {
                    text: ti.text + "!";
                    font-size: 12px;
                }
            }
        }
    }

    let ui = TestCase::new().unwrap();
    ui.show().unwrap();

    // Initial render so the layout settles.
    window.draw_if_needed(render);

    // Click inside the TextInput to give it focus.
    ui.window().dispatch_event(slint::platform::WindowEvent::PointerPressed {
        position: slint::LogicalPosition { x: 10.0, y: 16.0 },
        button: slint::platform::PointerEventButton::Left,
    });
    ui.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position: slint::LogicalPosition { x: 10.0, y: 16.0 },
        button: slint::platform::PointerEventButton::Left,
    });
    window.draw_if_needed(render);

    // Type a character. The KeyPressed handler runs `TextInput::set_cursor_position`,
    // which calls `Renderer::text_input_cursor_rect_for_byte_offset`. With the bug,
    // that function holds a `borrow_mut()` on font_context across `text_input.width()`,
    // and the dirty layout binding recurses into `text_size` → `borrow_mut` → panic.
    ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: "X".into() });
    ui.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: "X".into() });

    // One more render to flush any deferred work.
    window.draw_if_needed(render);
}
