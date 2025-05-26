// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Functions useful for testing
#![warn(missing_docs)]
#![allow(unsafe_code)]

use crate::api::LogicalPosition;
use crate::input::key_codes::Key;
use crate::platform::WindowEvent;

/// Slint animations do not use real time, but use a mocked time.
/// Normally, the event loop update the time of the animation using
/// real time, but in tests, it is more convenient to use the fake time.
/// This function will add some milliseconds to the fake time
#[unsafe(no_mangle)]
pub extern "C" fn slint_mock_elapsed_time(time_in_ms: u64) {
    let tick = crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
        let mut tick = driver.current_tick();
        tick += core::time::Duration::from_millis(time_in_ms);
        driver.update_animations(tick);
        tick
    });
    crate::timers::TimerList::maybe_activate_timers(tick);
    crate::properties::ChangeTracker::run_change_handlers();
}

/// Return the current mocked time.
#[unsafe(no_mangle)]
pub extern "C" fn slint_get_mocked_time() -> u64 {
    crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.current_tick()).as_millis()
}

/// Simulate a click on a position within the component.
#[unsafe(no_mangle)]
pub extern "C" fn slint_send_mouse_click(
    x: f32,
    y: f32,
    window_adapter: &crate::window::WindowAdapterRc,
) {
    let position = LogicalPosition::new(x, y);
    let button = crate::items::PointerEventButton::Left;

    window_adapter.window().dispatch_event(WindowEvent::PointerMoved { position });
    window_adapter.window().dispatch_event(WindowEvent::PointerPressed { position, button });
    slint_mock_elapsed_time(50);
    window_adapter.window().dispatch_event(WindowEvent::PointerReleased { position, button });
}

/// Simulate a character input event (pressed or released).
#[unsafe(no_mangle)]
pub extern "C" fn slint_send_keyboard_char(
    string: &crate::SharedString,
    pressed: bool,
    window_adapter: &crate::window::WindowAdapterRc,
) {
    for ch in string.chars() {
        window_adapter.window().dispatch_event(if pressed {
            WindowEvent::KeyPressed { text: ch.into() }
        } else {
            WindowEvent::KeyReleased { text: ch.into() }
        })
    }
}

/// Simulate a character input event.
#[unsafe(no_mangle)]
pub extern "C" fn send_keyboard_string_sequence(
    sequence: &crate::SharedString,
    window_adapter: &crate::window::WindowAdapterRc,
) {
    for ch in sequence.chars() {
        if ch.is_ascii_uppercase() {
            window_adapter
                .window()
                .dispatch_event(WindowEvent::KeyPressed { text: Key::Shift.into() });
        }

        let text: crate::SharedString = ch.into();
        window_adapter.window().dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window_adapter.window().dispatch_event(WindowEvent::KeyReleased { text });

        if ch.is_ascii_uppercase() {
            window_adapter
                .window()
                .dispatch_event(WindowEvent::KeyReleased { text: Key::Shift.into() });
        }
    }
}

/// implementation details for debug_log()
#[doc(hidden)]
pub fn debug_log_impl(args: core::fmt::Arguments) {
    crate::context::GLOBAL_CONTEXT.with(|p| match p.get() {
        Some(ctx) => ctx.platform().debug_log(args),
        None => default_debug_log(args),
    });
}

#[doc(hidden)]
pub fn default_debug_log(_arguments: core::fmt::Arguments) {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            use wasm_bindgen::prelude::*;
            use std::string::ToString;

            #[wasm_bindgen]
            extern "C" {
                #[wasm_bindgen(js_namespace = console)]
                pub fn log(s: &str);
            }

            log(&_arguments.to_string());
        } else if #[cfg(feature = "std")] {
            std::eprintln!("{_arguments}");
        }
    }
}

#[macro_export]
/// This macro allows producing debug output that will appear on stderr in regular builds
/// and in the console log for wasm builds.
macro_rules! debug_log {
    ($($t:tt)*) => ($crate::tests::debug_log_impl(format_args!($($t)*)))
}
