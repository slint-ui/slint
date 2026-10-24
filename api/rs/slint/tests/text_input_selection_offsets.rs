// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `TextInput::set_selection_offsets` restarts the cursor blink, so a cursor moved while it was
//! blinked off shows up at once, as with the arrow keys.

slint::slint! {
    export component TestCase inherits Window {
        width: 300phx;
        height: 100phx;
        out property <bool> input-focused: input.has-focus;
        public function select(anchor: int, focus: int) {
            input.set-selection-offsets(anchor, focus);
        }
        init => { input.focus(); }
        input := TextInput {
            text: "Lorem Ipsum";
            width: 100%;
            height: 100%;
        }
    }
}

#[test]
fn set_selection_offsets_shows_the_cursor() {
    i_slint_backend_testing::init_no_event_loop();
    let instance = TestCase::new().unwrap();
    assert!(instance.get_input_focused());
    let cursor_visible = || {
        let window = i_slint_core::window::WindowInner::from_pub(instance.window());
        let focus_item = window.focus_item.borrow().upgrade().unwrap();
        focus_item
            .downcast::<i_slint_core::items::TextInput>()
            .unwrap()
            .as_pin_ref()
            .cursor_visible()
    };
    assert!(cursor_visible());
    // Half of the default 1000 ms cycle, plus a little: the cursor is blinked off.
    i_slint_backend_testing::mock_elapsed_time(600);
    assert!(!cursor_visible());

    instance.invoke_select(2, 5);
    assert!(cursor_visible());
}
