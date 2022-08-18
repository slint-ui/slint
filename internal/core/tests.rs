// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Functions useful for testing
#![warn(missing_docs)]
#![allow(unsafe_code)]

use crate::input::{KeyEvent, KeyEventType, KeyboardModifiers, MouseEvent};
use crate::Coord;
use crate::SharedString;

/// Slint animations do not use real time, but use a mocked time.
/// Normally, the event loop update the time of the animation using
/// real time, but in tests, it is more convenient to use the fake time.
/// This function will add some milliseconds to the fake time
#[no_mangle]
pub extern "C" fn slint_mock_elapsed_time(time_in_ms: u64) {
    crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
        let mut tick = driver.current_tick();
        tick += core::time::Duration::from_millis(time_in_ms);
        driver.update_animations(tick)
    })
}

/// Simulate a click on a position within the component.
#[no_mangle]
pub extern "C" fn slint_send_mouse_click(
    component: &crate::component::ComponentRc,
    x: Coord,
    y: Coord,
    platform_window: &crate::window::PlatformWindowRc,
) {
    let mut state = crate::input::MouseInputState::default();
    let position = euclid::point2(x, y);

    state = crate::input::process_mouse_input(
        component.clone(),
        MouseEvent::Moved { position },
        platform_window,
        state,
    );
    state = crate::input::process_mouse_input(
        component.clone(),
        MouseEvent::Pressed { position, button: crate::items::PointerEventButton::Left },
        platform_window,
        state,
    );
    slint_mock_elapsed_time(50);
    crate::input::process_mouse_input(
        component.clone(),
        MouseEvent::Released { position, button: crate::items::PointerEventButton::Left },
        platform_window,
        state,
    );
}

/// Simulate a character input event.
#[no_mangle]
pub extern "C" fn send_keyboard_string_sequence(
    sequence: &crate::SharedString,
    modifiers: KeyboardModifiers,
    window: &crate::window::WindowInner,
) {
    for ch in sequence.chars() {
        let mut modifiers = modifiers;
        if ch.is_ascii_uppercase() {
            modifiers.shift = true;
        }
        let mut buffer = [0; 6];
        let text = SharedString::from(ch.encode_utf8(&mut buffer) as &str);

        window.process_key_input(&KeyEvent {
            event_type: KeyEventType::KeyPressed,
            text: text.clone(),
            modifiers,
        });
        window.process_key_input(&KeyEvent {
            event_type: KeyEventType::KeyReleased,
            text,
            modifiers,
        });
    }
}

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use wasm_bindgen::prelude::*;

        #[wasm_bindgen]
        extern "C" {
            #[wasm_bindgen(js_namespace = console)]
            pub fn log(s: &str);
        }

        #[macro_export]
        /// This macro allows producing debug output that will appear on stderr in regular builds
        /// and in the console log for wasm builds.
        macro_rules! debug_log {
            ($($t:tt)*) => ($crate::tests::log(&format_args!($($t)*).to_string()))
        }
    } else if #[cfg(feature = "std")] {
        #[doc(hidden)]
        pub use std::eprintln;

        /// This macro allows producing debug output that will appear on stderr in regular builds
        /// and in the console log for wasm builds.
        #[macro_export]
        macro_rules! debug_log {
            ($($t:tt)*) => ($crate::tests::eprintln!($($t)*))
        }
    } else if #[cfg(feature = "defmt")] {
        #[doc(hidden)]
        pub fn log(s: &str) {
            defmt::println!("{=str}", s);
        }

        #[macro_export]
        /// This macro allows producing debug output that will appear on the output of the debug probe
        macro_rules! debug_log {
            ($($t:tt)*) => ($crate::tests::log({ use alloc::string::ToString; &format_args!($($t)*).to_string() }))
        }
    }
}
