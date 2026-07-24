// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Tests for the `Window` focused-text clipboard API:
//! [`Window::copy_focused_text_selection`], [`Window::cut_focused_text_selection`], and
//! [`Window::paste_into_focused_text`] — the seam that lets custom backends service
//! copy/cut/paste from their own event handlers (e.g. web `ClipboardEvent`s).

mod common;

use slint::platform::{Key, WindowEvent};

const WIDTH: u32 = 200;
const HEIGHT: u32 = 32;

slint::slint! {
    export component TestCase inherits Window {
        width: 200px;
        height: 32px;
        in property <bool> ti-enabled: true;
        in property <bool> ti-read-only: false;
        in-out property <string> ti-text <=> ti.text;
        public function focus-ti() { ti.focus(); }
        ti := TextInput {
            enabled: root.ti-enabled;
            read-only: root.ti-read-only;
            font-size: 12px;
        }
    }
}

/// Extend the selection by `n` graphemes to the right, from the current cursor position.
fn select_right(window: &slint::Window, n: usize) {
    window.dispatch_event(WindowEvent::KeyPressed { text: Key::Shift.into() });
    for _ in 0..n {
        window.dispatch_event(WindowEvent::KeyPressed { text: Key::RightArrow.into() });
        window.dispatch_event(WindowEvent::KeyReleased { text: Key::RightArrow.into() });
    }
    window.dispatch_event(WindowEvent::KeyReleased { text: Key::Shift.into() });
}

/// Move the cursor `n` graphemes to the right without selecting.
fn move_right(window: &slint::Window, n: usize) {
    for _ in 0..n {
        window.dispatch_event(WindowEvent::KeyPressed { text: Key::RightArrow.into() });
        window.dispatch_event(WindowEvent::KeyReleased { text: Key::RightArrow.into() });
    }
}

#[test]
fn copy_returns_selection_and_cut_deletes_it() {
    common::setup(WIDTH, HEIGHT);
    let ui = TestCase::new().unwrap();
    ui.show().unwrap();
    ui.set_ti_text("Hello World".into());
    ui.invoke_focus_ti();

    // No selection yet: copy and cut return None, and cut deletes nothing.
    assert_eq!(ui.window().copy_focused_text_selection(), None);
    assert_eq!(ui.window().cut_focused_text_selection(), None);
    assert_eq!(ui.get_ti_text(), "Hello World");

    // Select "Hello" (cursor starts at 0).
    select_right(ui.window(), 5);

    // Copy returns the selection and leaves the text untouched.
    assert_eq!(ui.window().copy_focused_text_selection().as_deref(), Some("Hello"));
    assert_eq!(ui.get_ti_text(), "Hello World");

    // Cut returns the same selection and deletes it.
    assert_eq!(ui.window().cut_focused_text_selection().as_deref(), Some("Hello"));
    assert_eq!(ui.get_ti_text(), " World");
}

#[test]
fn paste_inserts_at_cursor_and_replaces_selection() {
    common::setup(WIDTH, HEIGHT);
    let ui = TestCase::new().unwrap();
    ui.show().unwrap();
    ui.set_ti_text("AB".into());
    ui.invoke_focus_ti();

    // Insert between "A" and "B"; the cursor ends up after the inserted text.
    move_right(ui.window(), 1);
    assert!(ui.window().paste_into_focused_text("X"));
    assert_eq!(ui.get_ti_text(), "AXB");

    // Select the trailing "B" and paste over it.
    select_right(ui.window(), 1);
    assert!(ui.window().paste_into_focused_text("Y"));
    assert_eq!(ui.get_ti_text(), "AXY");
}

#[test]
fn no_focused_text_input_refuses_everything() {
    common::setup(WIDTH, HEIGHT);
    let ui = TestCase::new().unwrap();
    ui.show().unwrap();
    ui.set_ti_text("text".into());
    // Nothing focused.
    assert_eq!(ui.window().copy_focused_text_selection(), None);
    assert_eq!(ui.window().cut_focused_text_selection(), None);
    assert!(!ui.window().paste_into_focused_text("nope"));
    assert_eq!(ui.get_ti_text(), "text");
}

#[test]
fn read_only_gates_cut_and_paste_but_not_copy() {
    common::setup(WIDTH, HEIGHT);
    let ui = TestCase::new().unwrap();
    ui.show().unwrap();
    ui.set_ti_text("Secret".into());
    ui.set_ti_read_only(true);
    ui.invoke_focus_ti();
    select_right(ui.window(), 6);

    // Copy is allowed on a read-only input, matching the Copy keyboard shortcut.
    assert_eq!(ui.window().copy_focused_text_selection().as_deref(), Some("Secret"));
    // Cut and paste refuse, matching the shortcuts' `!read-only` gate; nothing changes.
    assert_eq!(ui.window().cut_focused_text_selection(), None);
    assert!(!ui.window().paste_into_focused_text("overwrite"));
    assert_eq!(ui.get_ti_text(), "Secret");
}

#[test]
fn disabled_gates_cut_and_paste() {
    common::setup(WIDTH, HEIGHT);
    let ui = TestCase::new().unwrap();
    ui.show().unwrap();
    ui.set_ti_text("Frozen".into());
    ui.invoke_focus_ti();
    select_right(ui.window(), 6);
    // Disable after focusing and selecting.
    ui.set_ti_enabled(false);

    assert_eq!(ui.window().cut_focused_text_selection(), None);
    assert!(!ui.window().paste_into_focused_text("thaw"));
    assert_eq!(ui.get_ti_text(), "Frozen");
}

#[test]
fn has_focused_text_input_tracks_text_focus() {
    common::setup(WIDTH, HEIGHT);
    let ui = TestCase::new().unwrap();
    ui.show().unwrap();
    // Nothing focused yet.
    assert!(!ui.window().has_focused_text_input());
    // Focusing the TextInput reports true.
    ui.invoke_focus_ti();
    assert!(ui.window().has_focused_text_input());
}

#[test]
fn has_focused_text_input_false_for_non_text_focus() {
    slint::slint! {
        export component FocusCase inherits Window {
            width: 200px;
            height: 32px;
            public function focus-fs() { fs.focus(); }
            fs := FocusScope {}
        }
    }
    common::setup(WIDTH, HEIGHT);
    let ui = FocusCase::new().unwrap();
    ui.show().unwrap();
    // A focused non-text item (a FocusScope) is not a text input.
    ui.invoke_focus_fs();
    assert!(!ui.window().has_focused_text_input());
}

#[test]
fn multi_byte_selection_boundaries() {
    common::setup(WIDTH, HEIGHT);
    let ui = TestCase::new().unwrap();
    ui.show().unwrap();
    ui.set_ti_text("héllo".into()); // cspell:disable-line
    ui.invoke_focus_ti();

    // Select "hé" — the selection edge falls after a two-byte character.
    select_right(ui.window(), 2);
    assert_eq!(ui.window().copy_focused_text_selection().as_deref(), Some("hé"));
    assert_eq!(ui.window().cut_focused_text_selection().as_deref(), Some("hé"));
    assert_eq!(ui.get_ti_text(), "llo");

    // Paste multi-byte text back at the cursor.
    assert!(ui.window().paste_into_focused_text("→"));
    assert_eq!(ui.get_ti_text(), "→llo");
}
