// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![cfg_attr(feature = "mcu-board-support", no_std)]
#![cfg_attr(all(feature = "mcu-board-support", not(feature = "simulator")), no_main)]

#[cfg(feature = "mcu-board-support")]
extern crate alloc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub mod ui {
    slint::include_modules!();
}

use ui::*;

#[cfg(all(not(target_arch = "wasm32"), not(feature = "mcu-board-support")))]
mod controllers;

#[cfg(all(not(target_arch = "wasm32"), not(feature = "mcu-board-support")))]
use controllers::*;

#[cfg(not(feature = "mcu-board-support"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let window = MainWindow::new().unwrap();

    // let _ to keep the timer alive.
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "mcu-board-support")))]
    let _timer = header::setup(&window);

    #[cfg(feature = "network")]
    let weather_join = weather::setup(&window);

    window.run().unwrap();

    #[cfg(feature = "network")]
    weather_join.join().unwrap();
}

#[cfg(any(feature = "mcu-board-support", feature = "simulator"))]
#[mcu_board_support::entry]
fn main() -> ! {
    mcu_board_support::init();
    MainWindow::new().unwrap().run().unwrap();

    panic!("The MCU demo should not quit")
}
