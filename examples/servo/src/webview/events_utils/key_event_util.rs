// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::{
    SharedString,
    platform::Key as SlintKey,
    private_unstable_api::re_exports::{KeyEvent, KeyboardModifiers},
};

use servo::{Code, InputEvent, Key, KeyState, KeyboardEvent, Location, Modifiers, NamedKey};

pub fn convert_slint_key_event_to_servo_input_event(
    key_event: &KeyEvent,
    is_pressed: bool,
) -> InputEvent {
    let state = if is_pressed { KeyState::Down } else { KeyState::Up };
    let key = key_from_text(&key_event.text);
    let code = Code::Unidentified; // Slint doesn't provide physical key code
    let location = Location::Standard; // Slint doesn't provide key location
    let modifiers = get_modifiers(&key_event.modifiers);
    let keybord_event =
        KeyboardEvent::new_without_event(state, key, code, location, modifiers, false, false);
    InputEvent::Keyboard(keybord_event)
}

fn key_from_text(text: &str) -> Key {
    // Helper macro to check against a Slint Key
    macro_rules! check_key {
        ($slint_k:expr, $servo_k:expr) => {
            if text == SharedString::from($slint_k).as_str() {
                return Key::Named($servo_k);
            }
        };
    }

    check_key!(SlintKey::Backspace, NamedKey::Backspace);
    check_key!(SlintKey::Tab, NamedKey::Tab);
    check_key!(SlintKey::Return, NamedKey::Enter);
    check_key!(SlintKey::Escape, NamedKey::Escape);
    check_key!(SlintKey::Delete, NamedKey::Delete);

    // Modifiers
    check_key!(SlintKey::Shift, NamedKey::Shift);
    check_key!(SlintKey::ShiftR, NamedKey::Shift);
    check_key!(SlintKey::Control, NamedKey::Control);
    check_key!(SlintKey::ControlR, NamedKey::Control);
    check_key!(SlintKey::Alt, NamedKey::Alt);
    check_key!(SlintKey::AltGr, NamedKey::AltGraph);
    check_key!(SlintKey::Meta, NamedKey::Meta);
    check_key!(SlintKey::MetaR, NamedKey::Meta);

    // Arrow keys
    check_key!(SlintKey::UpArrow, NamedKey::ArrowUp);
    check_key!(SlintKey::DownArrow, NamedKey::ArrowDown);
    check_key!(SlintKey::LeftArrow, NamedKey::ArrowLeft);
    check_key!(SlintKey::RightArrow, NamedKey::ArrowRight);

    // F keys
    check_key!(SlintKey::F1, NamedKey::F1);
    check_key!(SlintKey::F2, NamedKey::F2);
    check_key!(SlintKey::F3, NamedKey::F3);
    check_key!(SlintKey::F4, NamedKey::F4);
    check_key!(SlintKey::F5, NamedKey::F5);
    check_key!(SlintKey::F6, NamedKey::F6);
    check_key!(SlintKey::F7, NamedKey::F7);
    check_key!(SlintKey::F8, NamedKey::F8);
    check_key!(SlintKey::F9, NamedKey::F9);
    check_key!(SlintKey::F10, NamedKey::F10);
    check_key!(SlintKey::F11, NamedKey::F11);
    check_key!(SlintKey::F12, NamedKey::F12);

    check_key!(SlintKey::End, NamedKey::End);
    check_key!(SlintKey::Home, NamedKey::Home);
    check_key!(SlintKey::Insert, NamedKey::Insert);
    check_key!(SlintKey::PageUp, NamedKey::PageUp);
    check_key!(SlintKey::PageDown, NamedKey::PageDown);
    check_key!(SlintKey::PageDown, NamedKey::PageDown);

    check_key!(SlintKey::Pause, NamedKey::Pause);
    check_key!(SlintKey::ScrollLock, NamedKey::ScrollLock);

    // If single character, return it
    if text.chars().count() == 1 {
        return Key::Character(text.to_string());
    }

    Key::Named(NamedKey::Unidentified)
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
