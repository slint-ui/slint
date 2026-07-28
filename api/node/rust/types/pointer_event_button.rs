// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::items::PointerEventButton;

/// This enum describes the different types of buttons for a pointer event, typically on a mouse or a pencil.
#[non_exhaustive]
#[napi(js_name = "PointerEventButton", string_enum)]
pub enum JsPointerEventButton {
    /// A button that is none of left, right, middle, back or forward. For example, this is used for the task button on a mouse with many buttons.
    Other,

    /// The left button.
    Left,
    /// The right button.
    Right,

    /// The center button.
    Middle,

    /// The back button.
    Back,

    /// The forward button.
    Forward,
}

impl From<JsPointerEventButton> for PointerEventButton {
    fn from(value: JsPointerEventButton) -> Self {
        match value {
            JsPointerEventButton::Other => PointerEventButton::Other,
            JsPointerEventButton::Left => PointerEventButton::Left,
            JsPointerEventButton::Right => PointerEventButton::Right,
            JsPointerEventButton::Middle => PointerEventButton::Middle,
            JsPointerEventButton::Back => PointerEventButton::Back,
            JsPointerEventButton::Forward => PointerEventButton::Forward,
        }
    }
}
