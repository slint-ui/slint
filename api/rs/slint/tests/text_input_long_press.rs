// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `TextInput::select_word_on_long_press`, driven the way the Android backend's long-press timer
//! drives it. Every glyph of the `FixedTestFont` is 10 pixels wide.

use slint::LogicalPosition;
use slint::platform::{PointerEventButton, WindowEvent};

slint::slint! {
    #[slint_edition = "1.19"]
    export component TestCase inherits Window {
        width: 300phx;
        height: 100phx;
        in-out property <string> test-text <=> input.text;
        out property <int> test-cursor-pos: input.cursor-position-byte-offset;
        out property <int> test-anchor-pos: input.anchor-position-byte-offset;
        out property <bool> input-focused: input.has-focus;
        init => { input.focus(); }
        input := TextInput {
            font-family: "FixedTestFont";
            font-size: 10px;
            width: 100%;
            height: 100%;
        }
    }
}

fn setup(text: &str) -> TestCase {
    i_slint_backend_testing::init_no_event_loop();
    let instance = TestCase::new().unwrap();
    assert!(instance.get_input_focused());
    instance.set_test_text(text.into());
    instance
}

/// What the Android backend calls when its long-press timer fires.
fn long_press(instance: &TestCase) {
    let window = i_slint_core::window::WindowInner::from_pub(instance.window());
    let focus_item = window.focus_item.borrow().upgrade().unwrap();
    let text_input = focus_item.downcast::<i_slint_core::items::TextInput>().unwrap();
    text_input.as_pin_ref().select_word_on_long_press(&window.window_adapter(), &focus_item);
}

fn press(instance: &TestCase, x: f32) {
    instance.window().dispatch_event(WindowEvent::PointerPressed {
        position: LogicalPosition::new(x, 6.0),
        button: PointerEventButton::Left,
    });
}

fn move_to(instance: &TestCase, x: f32) {
    instance
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, 6.0) });
}

fn selection(instance: &TestCase) -> (i32, i32) {
    (instance.get_test_anchor_pos(), instance.get_test_cursor_pos())
}

#[test]
fn small_move_keeps_the_word_and_dragging_extends_by_words() {
    let instance = setup("Lorem Ipsum Dolor Sit");
    press(&instance, 70.0);
    assert_eq!(selection(&instance), (7, 7));
    long_press(&instance);
    assert_eq!(selection(&instance), (6, 11));

    // The finger trembles inside the word.
    move_to(&instance, 75.0);
    assert_eq!(selection(&instance), (6, 11));

    move_to(&instance, 140.0);
    assert_eq!(selection(&instance), (6, 17));
    move_to(&instance, 75.0);
    assert_eq!(selection(&instance), (6, 11));
    move_to(&instance, 30.0);
    assert_eq!(selection(&instance), (11, 0));
}

#[test]
fn drift_into_the_next_word_before_the_timer() {
    let instance = setup("Lorem Ipsum Dolor Sit");
    press(&instance, 105.0);
    move_to(&instance, 125.0);
    assert_eq!(selection(&instance), (10, 12));
    long_press(&instance);
    // The next move after the long press shows what the user sees: the word where the press
    // started, extended to the word under the finger.
    move_to(&instance, 125.0);
    assert_eq!(selection(&instance), (6, 17));
}

#[test]
fn held_triple_press_keeps_its_paragraph() {
    let instance = setup("Hello world.\nNext");
    i_slint_backend_testing::send_mouse_click(&instance, 30.0, 6.0);
    i_slint_backend_testing::send_mouse_click(&instance, 30.0, 6.0);
    press(&instance, 30.0);
    assert_eq!(selection(&instance), (0, 12));
    long_press(&instance);
    assert_eq!(selection(&instance), (0, 12));
}

#[test]
fn nothing_is_selected_at_a_line_break() {
    let instance = setup("Hello\nWorld");
    press(&instance, 80.0);
    assert_eq!(selection(&instance), (5, 5));
    long_press(&instance);
    assert_eq!(selection(&instance), (5, 5));
}

#[test]
fn nothing_is_selected_at_the_end_of_the_text() {
    let instance = setup("Hello");
    press(&instance, 80.0);
    assert_eq!(selection(&instance), (5, 5));
    long_press(&instance);
    assert_eq!(selection(&instance), (5, 5));
}
