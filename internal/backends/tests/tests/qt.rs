// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod cases;

/// Make sure to call this at the start of each test case
fn init() {
    slint::platform::set_platform(Box::new(i_slint_backend_qt::Backend::new()) as Box<_>).ok(); // Already set
}
