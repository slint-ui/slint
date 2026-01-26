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

/// Simulate a click on a position within the component and releasing after some time.
/// The time until the release is hardcoded to 50ms
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

/// Simulate IME preedit (composition) text input.
///
/// This simulates the behavior of an IME setting preedit text on the focused TextInput.
/// The preedit text is displayed but not yet committed to the text field.
///
/// # Arguments
/// * `preedit` - The preedit/composition text to display (empty string clears preedit)
/// * `cursor` - Cursor position within the preedit text (byte offset), or None for end
#[cfg(feature = "std")]
pub fn simulate_ime_preedit(
    preedit: &str,
    cursor: Option<usize>,
    window_adapter: &crate::window::WindowAdapterRc,
) {
    use crate::items::TextInput;
    use crate::window::WindowInner;

    let window_inner = WindowInner::from_pub(window_adapter.window());

    // Get the focused item
    if let Some(focus_item) = window_inner.focus_item.borrow().upgrade() {
        // Check if it's a TextInput
        if let Some(text_input) = focus_item.downcast::<TextInput>() {
            if preedit.is_empty() {
                text_input.as_pin_ref().ime_clear_preedit(window_adapter, &focus_item);
            } else {
                text_input.as_pin_ref().ime_set_preedit(preedit, cursor, window_adapter, &focus_item);
            }
        }
    }
}

/// Simulate IME commit (finalize composition).
///
/// This simulates the behavior of an IME committing text, replacing any active preedit
/// with the final text.
///
/// # Arguments
/// * `text` - The text to commit
/// * `cursor_offset` - Where to place cursor relative to inserted text end
///   (0 = at end, negative = before, positive = after)
#[cfg(feature = "std")]
pub fn simulate_ime_commit(
    text: &str,
    cursor_offset: i32,
    window_adapter: &crate::window::WindowAdapterRc,
) {
    use crate::items::TextInput;
    use crate::window::WindowInner;

    let window_inner = WindowInner::from_pub(window_adapter.window());

    // Get the focused item
    if let Some(focus_item) = window_inner.focus_item.borrow().upgrade() {
        // Check if it's a TextInput
        if let Some(text_input) = focus_item.downcast::<TextInput>() {
            text_input.as_pin_ref().ime_commit_text(text, cursor_offset, window_adapter, &focus_item);
        }
    }
}

/// Simulate setting a composing region on existing text.
///
/// The composing region marks a range of existing committed text as "being edited" by the IME.
/// This is used by autocorrect features.
///
/// # Arguments
/// * `region` - The (start, end) byte offsets, or None to clear the region
#[cfg(feature = "std")]
pub fn simulate_ime_set_composing_region(
    region: Option<(usize, usize)>,
    window_adapter: &crate::window::WindowAdapterRc,
) {
    use crate::items::TextInput;
    use crate::window::WindowInner;

    let window_inner = WindowInner::from_pub(window_adapter.window());

    // Get the focused item
    if let Some(focus_item) = window_inner.focus_item.borrow().upgrade() {
        // Check if it's a TextInput
        if let Some(text_input) = focus_item.downcast::<TextInput>() {
            text_input.as_pin_ref().ime_set_composing_region(region, window_adapter, &focus_item);
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
