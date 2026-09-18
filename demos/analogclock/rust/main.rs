// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![no_std]
#![cfg_attr(not(feature = "simulator"), no_main)]

extern crate alloc;

use core::cell::Cell;

#[allow(unused_imports)]
use mcu_board_support::prelude::*;

slint::include_modules!();

#[mcu_board_support::entry]
fn main() -> ! {
    mcu_board_support::init();
    let main_window = MainWindow::new().unwrap();

    // Bare metal has no wall clock, so the demo starts from the classic
    // watch-face pose and counts up from there.
    let total_seconds = Cell::new((10 * 60 + 8) * 60 + 37u32);

    let timer = slint::Timer::default();
    let weak = main_window.as_weak();
    timer.start(slint::TimerMode::Repeated, core::time::Duration::from_secs(1), move || {
        let Some(window) = weak.upgrade() else { return };
        let now = (total_seconds.get() + 1) % (24 * 60 * 60);
        total_seconds.set(now);
        window.set_seconds((now % 60) as i32);
        window.set_minutes((now / 60 % 60) as i32);
        window.set_hours((now / 3600 % 12) as i32);
    });

    main_window.run().unwrap();

    panic!("The demo should not quit")
}
