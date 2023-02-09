// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial
use slint::{Timer, TimerMode};

slint::slint!(import { MainWindow } from "timer.slint";);

pub fn main() {
    let main_window = MainWindow::new().unwrap();
    let timer = Timer::default();
    {
        let main_window_weak = main_window.as_weak();
        timer.start(TimerMode::Repeated, std::time::Duration::from_millis(10), move || {
            let main_window = main_window_weak.unwrap();
            main_window.invoke_tick(10);
        });
    }
    main_window.run().unwrap();
}
