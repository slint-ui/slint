// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::items::PointerEventButton;

#[non_exhaustive]
#[napi(js_name = "PointerEventButton", string_enum = "lowercase")]
pub enum JsPointerEventButton {
    Other,
    Left,
    Right,
    Middle,
    Back,
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
