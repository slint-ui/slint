// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Functions useful for testing
#![warn(missing_docs)]
#![allow(unsafe_code)]

use crate::input::{key_codes::Key, KeyEventType, KeyInputEvent, MouseEvent};
use crate::window::WindowInner;
use crate::Coord;
use crate::SharedString;

/// Slint animations do not use real time, but use a mocked time.
/// Normally, the event loop update the time of the animation using
/// real time, but in tests, it is more convenient to use the fake time.
/// This function will add some milliseconds to the fake time
#[no_mangle]
pub extern "C" fn slint_mock_elapsed_time(time_in_ms: u64) {
    let tick = crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
        let mut tick = driver.current_tick();
        tick += core::time::Duration::from_millis(time_in_ms);
        driver.update_animations(tick);
        tick
    });
    crate::timers::TimerList::maybe_activate_timers(tick);
}

/// Simulate a click on a position within the component.
#[no_mangle]
pub extern "C" fn slint_send_mouse_click(
    component: &crate::component::ComponentRc,
    x: Coord,
    y: Coord,
    window_adapter: &crate::window::WindowAdapterRc,
) {
    let mut state = crate::input::MouseInputState::default();
    let position = euclid::point2(x, y);

    state = crate::input::process_mouse_input(
        component.clone(),
        MouseEvent::Moved { position },
        window_adapter,
        state,
    );
    state = crate::input::process_mouse_input(
        component.clone(),
        MouseEvent::Pressed { position, button: crate::items::PointerEventButton::Left },
        window_adapter,
        state,
    );
    slint_mock_elapsed_time(50);
    crate::input::process_mouse_input(
        component.clone(),
        MouseEvent::Released { position, button: crate::items::PointerEventButton::Left },
        window_adapter,
        state,
    );
}

/// Simulate a character input event (pressed or released).
#[no_mangle]
pub extern "C" fn slint_send_keyboard_char(
    string: &crate::SharedString,
    pressed: bool,
    window_adapter: &crate::window::WindowAdapterRc,
) {
    WindowInner::from_pub(window_adapter.window()).process_key_input(KeyInputEvent {
        event_type: if pressed { KeyEventType::KeyPressed } else { KeyEventType::KeyReleased },
        text: string.clone(),
        ..Default::default()
    });
}

/// Simulate a character input event.
#[no_mangle]
pub extern "C" fn send_keyboard_string_sequence(
    sequence: &crate::SharedString,
    window_adapter: &crate::window::WindowAdapterRc,
) {
    for ch in sequence.chars() {
        if ch.is_ascii_uppercase() {
            WindowInner::from_pub(window_adapter.window()).process_key_input(KeyInputEvent {
                event_type: KeyEventType::KeyPressed,
                text: Key::Shift.into(),
                ..Default::default()
            });
        }
        let text = SharedString::from(ch);

        WindowInner::from_pub(window_adapter.window()).process_key_input(KeyInputEvent {
            event_type: KeyEventType::KeyPressed,
            text: text.clone(),
            ..Default::default()
        });
        WindowInner::from_pub(window_adapter.window()).process_key_input(KeyInputEvent {
            event_type: KeyEventType::KeyReleased,
            text,
            ..Default::default()
        });

        if ch.is_ascii_uppercase() {
            WindowInner::from_pub(window_adapter.window()).process_key_input(KeyInputEvent {
                event_type: KeyEventType::KeyReleased,
                text: Key::Shift.into(),
                ..Default::default()
            });
        }
    }
}

/// implementation details for debug_log()
#[doc(hidden)]
pub fn debug_log_impl(args: core::fmt::Arguments) {
    crate::platform::PLATFORM_INSTANCE.with(|p| match p.get() {
        Some(platform) => platform.debug_log(args),
        None => default_debug_log(args),
    });
}

#[doc(hidden)]
pub fn default_debug_log(_arguments: core::fmt::Arguments) {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            use wasm_bindgen::prelude::*;

            #[wasm_bindgen]
            extern "C" {
                #[wasm_bindgen(js_namespace = console)]
                pub fn log(s: &str);
            }

            log(&_arguments.to_string());
        } else if #[cfg(feature = "std")] {
            eprintln!("{}", _arguments);
        }
    }
}

#[macro_export]
/// This macro allows producing debug output that will appear on stderr in regular builds
/// and in the console log for wasm builds.
macro_rules! debug_log {
    ($($t:tt)*) => ($crate::tests::debug_log_impl(format_args!($($t)*)))
}
