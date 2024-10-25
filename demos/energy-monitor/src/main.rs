// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
#![no_std]
#![cfg_attr(all(feature = "mcu-board-support", not(feature = "simulator")), no_main)]

#[cfg(not(feature = "mcu-board-support"))]
pub fn main() {
    energy_monitor::main();
}

#[cfg(feature = "mcu-board-support")]
#[mcu_board_support::entry]
fn main() -> ! {
    mcu_board_support::init();
    energy_monitor::main();
    panic!("The MCU demo should not quit")
}
