// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use i_slint_core::items::{KeyEvent, KeyboardModifiers};
use servo::{Code, Key, KeyState, KeyboardEvent, Location, Modifiers, NamedKey};

pub fn convert_slint_key_event_to_servo_keyboard_event(
    key_event: &KeyEvent,
    is_pressed: bool,
) -> KeyboardEvent {
    let state = if is_pressed { KeyState::Down } else { KeyState::Up };
    let key = key_from_text(&key_event.text);
    let code = Code::Unidentified; // Slint doesn't provide physical key code
    let location = Location::Standard; // Slint doesn't provide key location
    let modifiers = get_modifiers(&key_event.modifiers);
    KeyboardEvent::new_without_event(state, key, code, location, modifiers, false, false)
}

fn key_from_text(text: &str) -> Key {
    match text {
        // TODO: Add more mappings as needed
        t if t.chars().count() == 1 => Key::Character(t.to_string()),
        _ => Key::Named(NamedKey::Unidentified),
    }
}

fn get_modifiers(modifiers: &KeyboardModifiers) -> Modifiers {
    let mut mods = Modifiers::empty();
    if modifiers.control {
        mods.insert(Modifiers::CONTROL);
    }
    if modifiers.shift {
        mods.insert(Modifiers::SHIFT);
    }
    if modifiers.alt {
        mods.insert(Modifiers::ALT);
    }
    if modifiers.meta {
        mods.insert(Modifiers::META);
    }
    mods
}
