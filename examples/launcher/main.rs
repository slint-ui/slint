// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![no_std]
#![cfg_attr(not(feature = "mcu-simulator"), no_main)]

slint::include_modules!();

#[i_slint_backend_mcu::entry]
fn main() -> ! {
    i_slint_backend_mcu::init();

    let to_launch = {
        let main_window = Launcher::new();
        main_window.on_quit(slint::quit_event_loop);
        main_window.run();
        main_window.get_to_launch()
    };

    match to_launch {
        1 => printerdemo_mcu::run(),
        3 => memory::main(),
        _ => panic!("Cannot launch yet {to_launch}"),
    }

    panic!("shouldn't terminate");
}
