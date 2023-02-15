// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::ui::*;
use chrono::prelude::*;
use slint::*;

pub fn setup(window: &MainWindow) -> Timer {
    let update_timer = Timer::default();
    update_timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(300), {
        let weak_window = window.as_weak();

        move || {
            update(&weak_window.unwrap().global::<HeaderAdapter>());
        }
    });

    update_timer
}

fn update(header_adapter: &HeaderAdapter) {
    let now = Local::now();
    header_adapter.set_date(SharedString::from(now.format("%A %e %B %Y").to_string()));
    header_adapter.set_time(SharedString::from(now.format("%I:%M").to_string()));
    header_adapter.set_time_suffix(SharedString::from(now.format("%p").to_string()));
}
