// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![cfg_attr(feature = "mcu-board-support", no_std)]
#![cfg_attr(all(feature = "mcu-board-support", not(feature = "simulator")), no_main)]

#[cfg(feature = "mcu-board-support")]
extern crate alloc;

#[cfg(feature = "mcu-board-support")]
#[allow(unused_imports)]
use mcu_board_support::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

#[cfg(not(feature = "mcu-board-support"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    MainWindow::new().unwrap().run().unwrap();
}

#[cfg(any(feature = "mcu-board-support", feature = "simulator"))]
#[mcu_board_support::entry]
fn main() -> ! {
    mcu_board_support::init();
    MainWindow::new().unwrap().run().unwrap();

    panic!("The MCU demo should not quit")
}
