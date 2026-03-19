// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod cases;

fn init() {
    slint::platform::set_platform(
        Box::new(i_slint_backend_winit::Backend::new().unwrap()) as Box<_>
    )
    .expect("Failed to set platform!");
}
