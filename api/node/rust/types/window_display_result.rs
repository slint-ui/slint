// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::platform::WindowEventDispatchResult;

/// Result of dispatching a window event through Slint's runtime.
#[derive(Clone, Debug, PartialEq)]
#[napi(js_name = "WindowEventDispatchResult")]
pub enum JsWindowEventDispatchResult {
    /// The event was handled. For example, a key handler consumed a key press, or
    /// the window acted on a resize or close request.
    Accepted,
    /// The event wasn't handled: no element consumed it, or a handler actively refused it,
    /// such as a `close-requested` callback returning `reject` to keep the window open.
    Rejected,
}

impl From<WindowEventDispatchResult> for JsWindowEventDispatchResult {
    fn from(value: WindowEventDispatchResult) -> Self {
        match value {
            WindowEventDispatchResult::Accepted => JsWindowEventDispatchResult::Accepted,
            WindowEventDispatchResult::Rejected => JsWindowEventDispatchResult::Rejected,
            _ => JsWindowEventDispatchResult::Rejected,
        }
    }
}
