// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub fn use_24_hour_format() -> bool {
    true
}

#[cfg(feature = "ffi")]
mod ffi {
    #![allow(unsafe_code)]

    /// Perform the translation and formatting.
    #[no_mangle]
    pub extern "C" fn slint_use_24_hour_format() -> bool {
        super::use_24_hour_format()
    }
}
