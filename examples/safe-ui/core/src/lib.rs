// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![no_std]

// Enforce mutual exclusivity of pixel format
#[cfg(all(feature = "pixel-bgra8888", feature = "pixel-rgb565"))]
compile_error!("Cannot enable both pixel-bgra8888 and pixel-rgb565");

#[cfg(all(feature = "pixel-bgra8888", feature = "pixel-rgb888"))]
compile_error!("Cannot enable both pixel-bgra8888 and pixel-rgb888");

#[cfg(all(feature = "pixel-rgb565", feature = "pixel-rgb888"))]
compile_error!("Cannot enable both pixel-rgb565 and pixel-rgb888");

#[cfg(not(any(feature = "pixel-bgra8888", feature = "pixel-rgb565", feature = "pixel-rgb888")))]
compile_error!(
    "Must enable exactly one pixel format: pixel-bgra8888, pixel-rgb565 or pixel-rgb888"
);

extern crate alloc;

pub mod pixels;
pub mod platform;

slint::include_modules!();

#[unsafe(no_mangle)]
pub extern "C" fn slint_app_main() {
    platform::slint_init_safeui_platform();

    let app = MainWindow::new().unwrap();

    app.show().unwrap();

    app.run().unwrap();
}
