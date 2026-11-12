// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub(crate) fn create(window: Option<slint::WindowHandle>) -> rfd::FileDialog {
    let dialog = rfd::FileDialog::new();
    match window {
        Some(window) => dialog.set_parent(&window),
        None => dialog,
    }
}
