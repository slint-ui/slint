// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
#![no_std]
#![cfg_attr(all(feature = "mcu-board-support", not(feature = "simulator")), no_main)]

#[cfg(not(feature = "mcu-board-support"))]
pub fn main() {
    energy_monitor::main();
}

#[cfg(all(
    any(feature = "mcu-board-support", feature = "simulator"),
    not(feature = "from_launcher")
))]
#[mcu_board_support::entry]
fn main() -> ! {
    mcu_board_support::init();
    energy_monitor::main();
    panic!("The MCU demo should not quit")
}

#[cfg(feature = "from_launcher")]
pub fn main() -> ! {
    let window = MainWindow::new().unwrap();

    let _kiosk_mode_timer = kiosk_timer(&window);

    window.run().unwrap();
    panic!("The MCU demo should not quit")
}
