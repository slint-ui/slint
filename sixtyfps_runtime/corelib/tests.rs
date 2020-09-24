/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Functions usefull for testing
#![warn(missing_docs)]

use crate::input::{MouseEvent, MouseEventType};

/// SixtyFPS animations do not use real time, but use a mocked time.
/// Normally, the event loop update the time of the animation using
/// real time, but in tests, it is more convinient to use the fake time.
/// This function will add some milliseconds to the fake time
#[no_mangle]
pub extern "C" fn sixtyfps_mock_elapsed_time(time_in_ms: u64) {
    crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
        let mut tick = driver.current_tick();
        tick += instant::Duration::from_millis(time_in_ms);
        driver.update_animations(tick)
    })
}

/// Simulate a click on a position within the component.
#[no_mangle]
pub extern "C" fn sixtyfps_send_mouse_click(
    component: core::pin::Pin<crate::component::ComponentRef>,
    x: f32,
    y: f32,
    window: &crate::eventloop::ComponentWindow,
) {
    let pos = euclid::point2(x, y);
    component.as_ref().input_event(
        MouseEvent { pos, what: MouseEventType::MouseMoved },
        window,
        &component,
    );
    component.as_ref().input_event(
        MouseEvent { pos, what: MouseEventType::MousePressed },
        window,
        &component,
    );
    sixtyfps_mock_elapsed_time(50);
    component.as_ref().input_event(
        MouseEvent { pos, what: MouseEventType::MouseReleased },
        window,
        &component,
    );
}

/// Simulate a change in keyboard modifiers pressed.
#[no_mangle]
pub extern "C" fn sixtyfps_set_keyboard_modifiers(
    window: &crate::eventloop::ComponentWindow,
    modifiers: crate::input::KeyboardModifiers,
) {
    window.set_current_keyboard_modifiers(modifiers)
}

/// Simulate a key down event.
#[no_mangle]
pub extern "C" fn sixtyfps_send_key_clicks(
    component: core::pin::Pin<crate::component::ComponentRef>,
    key_codes: &crate::slice::Slice<crate::input::KeyCode>,
    window: &crate::eventloop::ComponentWindow,
) {
    for key_code in key_codes.iter() {
        window.process_key_input(
            &crate::input::KeyEvent::KeyPressed {
                code: *key_code,
                modifiers: window.current_keyboard_modifiers(),
            },
            component,
        );
        window.process_key_input(
            &crate::input::KeyEvent::KeyReleased {
                code: *key_code,
                modifiers: window.current_keyboard_modifiers(),
            },
            component,
        );
    }
}

/// Simulate a character input event.
#[no_mangle]
pub extern "C" fn send_keyboard_string_sequence(
    component: core::pin::Pin<crate::component::ComponentRef>,
    sequence: &crate::SharedString,
    window: &crate::eventloop::ComponentWindow,
) {
    use std::convert::TryInto;

    let key_down = |maybe_code: &Option<crate::input::KeyCode>| {
        maybe_code.clone().map(|code| {
            window.process_key_input(
                &crate::input::KeyEvent::KeyPressed {
                    code: code,
                    modifiers: window.current_keyboard_modifiers(),
                },
                component,
            );
        });
    };

    let key_up = |maybe_code: &Option<crate::input::KeyCode>| {
        maybe_code.clone().map(|code| {
            window.process_key_input(
                &crate::input::KeyEvent::KeyReleased {
                    code: code,
                    modifiers: window.current_keyboard_modifiers(),
                },
                component,
            );
        });
    };

    for ch in sequence.chars() {
        let maybe_key_code = if ch.is_ascii_uppercase() {
            window.set_current_keyboard_modifiers(crate::input::SHIFT_MODIFIER.into());
            ch.to_ascii_lowercase().try_into()
        } else {
            ch.try_into()
        }
        .ok();

        key_down(&maybe_key_code);

        window.process_key_input(
            &crate::input::KeyEvent::CharacterInput {
                unicode_scalar: ch.into(),
                modifiers: window.current_keyboard_modifiers(),
            },
            component,
        );

        key_up(&maybe_key_code);

        window.set_current_keyboard_modifiers(crate::input::NO_MODIFIER.into());
    }
}
