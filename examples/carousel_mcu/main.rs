// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![no_std]
#![cfg_attr(not(feature = "simulator"), no_main)]

extern crate alloc;

slint::include_modules!();


#[mcu_board_support::entry]
fn main() -> ! {
    mcu_board_support::init();
    let main_window = MainWindow::new();
  
    main_window.run();

    panic!("The MCU demo should not quit")
}
