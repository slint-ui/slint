// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module provides access to the system clipboard.
*/

use alloc::string::String;

// TODO: We can't connect to the wayland clipboard yet because
// it requires an external connection.
#[cfg(feature = "clipboard")]
cfg_if::cfg_if! {
    if #[cfg(all(
            unix,
            not(any(
                target_os = "macos",
                target_os = "android",
                target_os = "ios",
                target_os = "emscripten"
            )),
            not(feature = "x11")
        ))] {
        type ClipboardBackend = copypasta::nop_clipboard::NopClipboardContext;
    } else {
        type ClipboardBackend = copypasta::ClipboardContext;
    }
}

#[cfg(feature = "clipboard")]
thread_local!(pub(crate) static CLIPBOARD : core::cell::RefCell<ClipboardBackend>  = std::cell::RefCell::new(ClipboardBackend::new().unwrap()));

/// Writes text into the system clipboard.
pub fn set_clipboard_text(_text: String) {
    #[cfg(feature = "clipboard")]
    {
        use copypasta::ClipboardProvider;
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().set_contents(_text).ok());
    }
}

/// Returns the text that is stored in the system clipboard, if any.
pub fn clipboard_text() -> Option<String> {
    #[cfg(feature = "clipboard")]
    {
        use copypasta::ClipboardProvider;
        return CLIPBOARD.with(|clipboard| clipboard.borrow_mut().get_contents().ok());
    }
    #[cfg(not(feature = "clipboard"))]
    None
}
