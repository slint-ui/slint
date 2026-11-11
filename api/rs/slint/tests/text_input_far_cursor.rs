// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The software renderer stores coordinates as `i16`. The cursor of a long text input, scrolled
//! more than 32767 physical pixels out of view, used to panic in the cast.

mod common;

const WIDTH: usize = 100;
const HEIGHT: usize = 100;

#[test]
fn far_away_cursor_does_not_panic() {
    let window = common::setup(WIDTH as u32, HEIGHT as u32);

    slint::slint! {
        #[edition = "1.20"]
        export component TestCase inherits Window {
            width: 100px;
            height: 100px;
            in property <string> text;
            in property <int> end;
            fl := Flickable {
                content-height: ti.height;
                ti := TextInput {
                    text: root.text;
                    width: parent.width;
                    height: self.preferred-height;
                    font-size: 12px;
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

    let ui = TestCase::new().unwrap();
    let text = "line\n".repeat(5000);
    ui.set_end(text.len() as i32);
    ui.set_text(text.into());
    ui.show().unwrap();

    let mut buffer = vec![common::TestPixel::default(); WIDTH * HEIGHT];
    let mut render = || {
        window.request_redraw();
        window.draw_if_needed(|renderer| {
            renderer.render(&mut buffer, WIDTH);
        });
    };
    render();
    // Moving the cursor scrolls it into view; scroll back so it ends up far below the window.
    ui.invoke_cursor_to_end();
    render();
    ui.invoke_scroll_to_top();
    render();
}
