// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint_interpreter::WindowEventDispatchResult;

/// Result of dispatching a window event through Slint's runtime.
#[derive(Clone, Debug, PartialEq)]
#[napi(js_name = "WindowEventDispatchResult")]
pub enum JsWindowEventDispatchResult {
    /// The event was handled. For example, a key handler consumed a key press, or
    /// the window acted on a resize or close request.
    Accepted,
    /// The event was actively refused. For example, a `close-requested` callback
    /// returned `reject` to prevent the window from closing.
    Rejected,
    /// The event was not handled by any element.
    Ignored,
}

impl From<WindowEventDispatchResult> for JsWindowEventDispatchResult {
    fn from(value: WindowEventDispatchResult) -> Self {
        match value {
            WindowEventDispatchResult::Accepted => JsWindowEventDispatchResult::Accepted,
            WindowEventDispatchResult::Rejected => JsWindowEventDispatchResult::Rejected,
            WindowEventDispatchResult::Ignored => JsWindowEventDispatchResult::Ignored,
            _ => JsWindowEventDispatchResult::Ignored,
        }
    }
}
