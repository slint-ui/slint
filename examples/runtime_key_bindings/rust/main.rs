// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Demonstrates creating and assigning `Keys` values at runtime from Rust.
//!
//! Key bindings are normally defined at compile time with `@keys(...)` in `.slint` files.
//! With `Keys::from_parts`, you can create them at runtime — useful for user-configurable
//! shortcuts loaded from a config file or database.
//!
//! This example also shows how to capture a key event and convert it into a
//! `Keys` value, enabling graphical shortcut configuration.

use slint::Keys;

slint::include_modules!();

fn main() {
    let window = MainWindow::new().unwrap();

    let window_weak = window.as_weak();
    window.on_shortcut_activated(move |action| {
        let window = window_weak.upgrade().unwrap();
        match action.as_str() {
            "save" => println!("Save"),
            "undo" => println!("Undo"),
            "user" => println!("User shortcut ({})", window.get_user_shortcut()),
            "reassign-ctrl-p" => {
                let keys = Keys::from_parts(["Control", "P"]).unwrap();
                println!("Reassigned to {keys}");
                window.set_user_shortcut(keys);
            }
            _ => {}
        }
    });

    // Capture a key event and turn it into a Keys value.
    // This enables graphical configuration of keyboard shortcuts.
    window.on_key_event({
        let window = window.as_weak();
        move |event| {
            let window = window.upgrade().unwrap();
            let mut parts = Vec::new();
            if event.modifiers.control {
                parts.push("Control");
            }
            if event.modifiers.alt {
                parts.push("Alt");
            }
            if event.modifiers.shift {
                parts.push("Shift");
            }
            if event.modifiers.meta {
                parts.push("Meta");
            }
            parts.push(&event.text);
            match Keys::from_parts(parts.iter().copied()) {
                Ok(keys) => {
                    println!("Captured shortcut: {keys}");
                    window.set_user_shortcut(keys);
                }
                Err(e) => eprintln!("Invalid shortcut: {e}"),
            }
        }
    });

    println!("Press Ctrl+S, Ctrl+Z, or Ctrl+E (default user shortcut)");
    println!("Click 'Capture shortcut' then press a key combo to reassign");

    window.run().unwrap();
}
