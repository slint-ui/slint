// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![no_std]

extern crate alloc;

pub mod platform;

slint::include_modules!();

#[unsafe(no_mangle)]
pub extern "C" fn slint_app_main() {
    platform::slint_init_safeui_platform();

    let app = MainWindow::new().unwrap();

    app.show().unwrap();

    app.run().unwrap();
}
