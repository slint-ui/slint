// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![no_std]
#![cfg_attr(not(feature = "mcu-simulator"), no_main)]

slint::include_modules!();

#[mcu_board_support::entry]
fn main() -> ! {
    mcu_board_support::init();

    let to_launch = {
        let main_window = Launcher::new().unwrap();
        main_window.on_quit(|| slint::quit_event_loop().unwrap());
        main_window.run().unwrap();
        main_window.get_to_launch()
    };

    match to_launch {
        1 => printerdemo_mcu::run(),
        2 => slide_puzzle::main(),
        3 => memory::main(),
        _ => panic!("Cannot launch yet {to_launch}"),
    }

    panic!("shouldn't terminate");
}
