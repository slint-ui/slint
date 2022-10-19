// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![cfg_attr(feature = "mcu", no_std)]
#![cfg_attr(all(feature = "mcu", not(simulator)), no_main)]

#[cfg(feature = "mcu")]
extern crate alloc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

#[cfg(not(feature = "mcu"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    MainWindow::new().run()
}

#[cfg(feature = "mcu")]
#[mcu_board_support::entry]
fn main() -> ! {
    mcu_board_support::init();
    MainWindow::new().run();

    panic!("The MCU demo should not quit")
}