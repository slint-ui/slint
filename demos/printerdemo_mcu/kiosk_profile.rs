// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::{Kiosk, MainWindow, kiosk};
use alloc::rc::Rc;
use core::cell::Cell;
use slint::ComponentHandle;

pub fn start(window: &MainWindow, controller: &Rc<kiosk::Controller>) -> slint::Timer {
    let timer = slint::Timer::default();
    let weak = window.as_weak();
    let stage = controller.stage.clone();
    let start = mcu_board_support::profile_now_us();
    let enabled = Cell::new(false);
    let done = Cell::new(false);
    timer.start(slint::TimerMode::Repeated, core::time::Duration::from_millis(1), move || {
        let Some(window) = weak.upgrade() else { return };
        let elapsed = mcu_board_support::profile_now_us() - start;
        if elapsed < 3_000_000 || done.get() {
            return;
        }
        if !enabled.replace(true) {
            window.global::<Kiosk>().set_enabled(true);
        }
        let elapsed = elapsed - 3_000_000;
        if elapsed >= kiosk::scene_seconds() * 3_000_000 {
            mcu_board_support::profile_mode(0);
            mcu_board_support::dispatch_touch_event(
                window.window(),
                slint::LogicalPosition::new(300., 20.),
                mcu_board_support::TouchPhase::Started,
            );
            assert!(!window.global::<Kiosk>().get_enabled(), "touch must exit Kiosk");
            mcu_board_support::dispatch_touch_event(
                window.window(),
                slint::LogicalPosition::new(300., 20.),
                mcu_board_support::TouchPhase::Ended,
            );
            done.set(true);
            mcu_board_support::profile_capture();
            return;
        }
        let phase = elapsed % (kiosk::scene_seconds() * 1_000_000);
        mcu_board_support::profile_mode(if (1_000_000..11_000_000).contains(&phase) {
            stage.get() as u32 + 1
        } else {
            0
        });
    });
    timer
}
