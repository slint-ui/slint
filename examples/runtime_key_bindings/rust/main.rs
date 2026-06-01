// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Demonstrates creating and assigning `Keys` values at runtime from Rust.
//!
//! Key bindings are normally defined at compile time with `@keys(...)` in `.slint` files.
//! With `Keys::from_parts`, you can create them at runtime — useful for user-configurable
//! shortcuts loaded from a config file or database.
//!
//! `Keys::to_parts` is the inverse: it lets you persist a customized shortcut back to
//! disk in a stable, human-readable, cross-platform form (the same parts list that
//! `from_parts` accepts).
//!
//! This example also shows how to capture a key event and convert it into a
//! `Keys` value, enabling graphical shortcut configuration.

use slint::Keys;

slint::include_modules!();

/// Path of the file used to persist the user's customized shortcut. Lives next
/// to the example sources so the saved value follows the example regardless of
/// the working directory the binary is launched from.
const CONFIG_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../user_shortcut.conf");

/// Load the first shortcut from `CONFIG_PATH`, if present. The file format is
/// one shortcut per line; parts are whitespace-separated and `#` introduces a
/// comment. Blank lines are ignored.
fn load_user_shortcut() -> Option<Keys> {
    let contents = std::fs::read_to_string(CONFIG_PATH).ok()?;
    for line in contents.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        return Keys::from_parts(parts.iter().copied()).ok();
    }
    None
}

/// Persist `keys` to `CONFIG_PATH` as a single line with whitespace-separated
/// parts. `to_parts` round-trips losslessly through `from_parts`, so the saved
/// file reloads into an equivalent `Keys` value on the next run.
fn save_user_shortcut(keys: &Keys) {
    let line: Vec<String> = keys.to_parts().into_iter().map(|p| p.to_string()).collect();
    let contents = format!(
        "# User shortcut for the runtime_key_bindings example.\n\
         # One shortcut per line; parts are whitespace-separated.\n\
         {}\n",
        line.join(" ")
    );
    if let Err(e) = std::fs::write(CONFIG_PATH, contents) {
        eprintln!("Failed to save shortcut to {CONFIG_PATH}: {e}");
    } else {
        println!("Saved shortcut to {CONFIG_PATH}");
    }
}

fn main() {
    let window = MainWindow::new().unwrap();

    // Restore the previously saved shortcut, if any. Falls back to the default
    // `@keys(Control + E)` baked into the .slint file when no config exists yet.
    if let Some(keys) = load_user_shortcut() {
        println!("Loaded shortcut from {CONFIG_PATH}: {keys}");
        window.set_user_shortcut(keys);
    }

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
                window.set_user_shortcut(keys.clone());
                save_user_shortcut(&keys);
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
                    window.set_user_shortcut(keys.clone());
                    save_user_shortcut(&keys);
                }
                Err(e) => eprintln!("Invalid shortcut: {e}"),
            }
        }
    });

    println!("Press Ctrl+S, Ctrl+Z, or Ctrl+E (default user shortcut)");
    println!("Click 'Capture shortcut' then press a key combo to reassign");
    println!("Reassigned shortcut is saved to {CONFIG_PATH} and restored on next launch");

    window.run().unwrap();
}
