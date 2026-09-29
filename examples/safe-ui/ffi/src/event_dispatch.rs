// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::{ffi_event::FfiEvent, ffi_event::FfiEventTag, platform};

use FfiEventTag::{
    FfiEventTag_PointerPressed as PointerPressed, FfiEventTag_PointerReleased as PointerReleased,
    FfiEventTag_Quit as Quit,
};

/// What the event loop should do with an incoming FFI event. Slint SC only
/// knows press and release; the other events the C layer may send are accepted
/// at the FFI boundary but ignored here.
pub enum EventAction {
    Quit,
    Touch(slint_sc::TouchEvent),
    Ignore,
}

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

/// Interpret a raw [`FfiEvent`] for the scene. Coordinates are physical pixels,
/// which the scene renders one-to-one.
pub fn convert_ffi_event(raw: &FfiEvent) -> EventAction {
    let position = slint_sc::Point::new(raw.payload.pos_x, raw.payload.pos_y);
    match raw.tag {
        Quit => EventAction::Quit,
        PointerPressed => EventAction::Touch(slint_sc::TouchEvent::pressed(position)),
        PointerReleased => EventAction::Touch(slint_sc::TouchEvent::released(position)),
        _ => EventAction::Ignore,
    }
}
