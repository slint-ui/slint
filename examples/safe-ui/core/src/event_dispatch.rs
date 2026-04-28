// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::{
    ffi_event::{FfiEvent, FfiEventTag, FfiPointerButton},
    platform,
};

use FfiEventTag::{
    FfiEventTag_KeyPressRepeated as KeyPressRepeated, FfiEventTag_KeyPressed as KeyPressed,
    FfiEventTag_KeyReleased as KeyReleased, FfiEventTag_PointerExited as PointerExited,
    FfiEventTag_PointerMoved as PointerMoved, FfiEventTag_PointerPressed as PointerPressed,
    FfiEventTag_PointerReleased as PointerReleased, FfiEventTag_PointerScrolled as PointerScrolled,
    FfiEventTag_Quit as Quit, FfiEventTag_Resized as Resized,
};

use FfiPointerButton::{
    FfiPointerButton_Left as Left, FfiPointerButton_Middle as Middle,
    FfiPointerButton_Other as Other, FfiPointerButton_Right as Right,
};

/// Push an input event into the queue from any execution context.
///
/// This function is the **only** FFI entry point for input events. It is
/// ISR-safe: no heap allocation, no blocking, no FPU usage.
#[unsafe(no_mangle)]
pub extern "C" fn slint_safeui_dispatch_event(raw: *const FfiEvent) -> i32 {
    if raw.is_null() {
        return -1;
    }

    // SAFETY: `raw` was checked for null above. Caller guarantees it points
    // to an initialized, properly aligned `FfiEvent`. We copy immediately;
    // no reference escapes.
    let event = unsafe { *raw };
    platform::push_input_event(event)
}

/// Convert a raw [`FfiEvent`] into a Slint `WindowEvent`, applying the
/// given scale factor for physical-to-logical coordinate conversion.
///
/// A `Quit` tag is returned as `None`; the caller should exit the event
/// loop.
pub fn convert_ffi_event(raw: &FfiEvent, scale: f32) -> Option<slint::platform::WindowEvent> {
    use slint::platform::WindowEvent;
    use slint::{PhysicalPosition, PhysicalSize};

    match raw.tag {
        Quit => None,

        PointerPressed => Some(WindowEvent::PointerPressed {
            position: PhysicalPosition::new(raw.payload.pos_x, raw.payload.pos_y).to_logical(scale),
            button: convert_button(raw.payload.button),
        }),

        PointerReleased => Some(WindowEvent::PointerReleased {
            position: PhysicalPosition::new(raw.payload.pos_x, raw.payload.pos_y).to_logical(scale),
            button: convert_button(raw.payload.button),
        }),

        PointerMoved => Some(WindowEvent::PointerMoved {
            position: PhysicalPosition::new(raw.payload.pos_x, raw.payload.pos_y).to_logical(scale),
        }),

        PointerScrolled => Some(WindowEvent::PointerScrolled {
            position: PhysicalPosition::new(raw.payload.pos_x, raw.payload.pos_y).to_logical(scale),
            // Scroll deltas are unitless — passed through without scaling.
            delta_x: raw.payload.delta_x,
            delta_y: raw.payload.delta_y,
        }),

        PointerExited => Some(WindowEvent::PointerExited),

        KeyPressed => {
            Some(WindowEvent::KeyPressed { text: key_code_to_shared_string(raw.payload.key_code) })
        }

        KeyPressRepeated => Some(WindowEvent::KeyPressRepeated {
            text: key_code_to_shared_string(raw.payload.key_code),
        }),

        KeyReleased => {
            Some(WindowEvent::KeyReleased { text: key_code_to_shared_string(raw.payload.key_code) })
        }

        Resized => Some(WindowEvent::Resized {
            size: PhysicalSize::new(
                raw.payload.width.max(0) as u32,
                raw.payload.height.max(0) as u32,
            )
            .to_logical(scale),
        }),
    }
}

fn convert_button(button: FfiPointerButton) -> slint::platform::PointerEventButton {
    use slint::platform::PointerEventButton;
    match button {
        Left => PointerEventButton::Left,
        Right => PointerEventButton::Right,
        Middle => PointerEventButton::Middle,
        Other => PointerEventButton::Other,
    }
}

/// Convert a Unicode code point (u32) to a `SharedString` for Slint key events.
///
/// Invalid code points (surrogates, values > U+10FFFF) are silently converted
/// to the Unicode replacement character U+FFFD rather than producing an empty
/// string. This makes invalid input visible during debugging instead of
/// silently swallowing events.
fn key_code_to_shared_string(code: u32) -> slint::SharedString {
    let c = char::from_u32(code).unwrap_or('\u{FFFD}');
    let mut buf = [0u8; 4];
    let s: &str = c.encode_utf8(&mut buf);
    slint::SharedString::from(s)
}
