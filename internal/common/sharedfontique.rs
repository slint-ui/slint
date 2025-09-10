// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use fontique;

thread_local! {
    pub static COLLECTION: std::cell::RefCell<fontique::Collection> = Default::default()
}
