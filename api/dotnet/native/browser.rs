// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The browser, through Slint's Emscripten backend.

use std::ffi::c_void;

use crate::guard_bool;

/// The selector has no Emscripten backend, so `SLINT_BACKEND=testing` is the only other choice.
pub(crate) fn install_platform() {
    if std::env::var("SLINT_BACKEND").is_ok_and(|b| b == "testing") {
        return;
    }
    if let Ok(platform) = i_slint_backend_emscripten::create_platform() {
        let _ = i_slint_core::platform::set_platform(platform);
    }
}

/// Starts the event loop and returns, since the browser's thread can't block.
/// `on_quit` runs once with `user_data` when the loop stops.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_browser_start_event_loop(
    on_quit: unsafe extern "C" fn(*mut c_void),
    user_data: *mut c_void,
) -> bool {
    guard_bool(|| {
        i_slint_backend_selector::with_platform(|_| Ok(())).map_err(|e| e.to_string())?;
        i_slint_backend_emscripten::start_event_loop(Box::new(move || unsafe {
            on_quit(user_data)
        }));
        Ok(())
    })
}
