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

/// Path of the file used to persist the user's customized shortcut.
const CONFIG_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../user_shortcut.conf");

/// Name of the action this example persists
const USER_ACTION: &str = "user";

/// Header written when the config file is created from scratch.
const CONFIG_HEADER: &[&str] = &[
    "// runtime_key_bindings - user shortcuts.",
    "// One per line: <action> <part> <part> ...",
    "// Parts come from `Keys::to_parts`; non-printable ones are percent-encoded.",
];

/// Percent-encode a part so it survives a whitespace-separated text file.
fn encode_part(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    for c in part.chars() {
        if c.is_ascii_graphic() && c != '%' {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).as_bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// Inverse of [`encode_part`]; `None` if the escaping is malformed.
fn decode_part(part: &str) -> Option<String> {
    let bytes = part.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            out.push(u8::from_str_radix(part.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Split a config line into its action name and its (still encoded) shortcut
/// parts, or `None` for blank lines and comments.
fn parse_line(line: &str) -> Option<(&str, Vec<&str>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") {
        return None;
    }
    let mut tokens = line.split_whitespace();
    let action = tokens.next()?;
    Some((action, tokens.collect()))
}

/// Load the shortcut stored for `action`, if `CONFIG_PATH` has a line for it.
fn load_shortcut(action: &str) -> Option<Keys> {
    let contents = std::fs::read_to_string(CONFIG_PATH).ok()?;
    let encoded = contents.lines().find_map(|line| match parse_line(line) {
        Some((name, parts)) if name == action => Some(parts),
        _ => None,
    })?;
    let parts: Vec<String> = encoded.iter().map(|p| decode_part(p)).collect::<Option<_>>()?;
    Keys::from_parts(parts.iter().map(|s| s.as_str())).ok()
}

/// Persist `keys` for `action`, leaving every other line of the file intact
fn save_shortcut(action: &str, keys: &Keys) {
    let parts: Vec<String> = keys.to_parts().map(encode_part).collect();
    let new_line = format!("{action} {}", parts.join(" "));

    // Rewrite this action's line in place and keep the rest, so the example does
    // not clobber a file holding more shortcuts than the one it owns.
    let mut lines: Vec<String> = Vec::new();
    let mut replaced = false;
    match std::fs::read_to_string(CONFIG_PATH) {
        Ok(contents) => {
            for line in contents.lines() {
                match parse_line(line) {
                    Some((name, _)) if name == action => {
                        lines.push(new_line.clone());
                        replaced = true;
                    }
                    _ => lines.push(line.to_string()),
                }
            }
        }
        Err(_) => lines.extend(CONFIG_HEADER.iter().map(|l| l.to_string())),
    }
    if !replaced {
        lines.push(new_line);
    }

    if let Err(e) = std::fs::write(CONFIG_PATH, lines.join("\n") + "\n") {
        eprintln!("Failed to save shortcut to {CONFIG_PATH}: {e}");
    } else {
        println!("Saved shortcut to {CONFIG_PATH}");
    }
}

fn main() {
    let window = MainWindow::new().unwrap();

    if let Some(keys) = load_shortcut(USER_ACTION) {
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
                save_shortcut(USER_ACTION, &keys);
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
                    save_shortcut(USER_ACTION, &keys);
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
