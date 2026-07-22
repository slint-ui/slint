// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![cfg_attr(feature = "mcu-board-support", no_std)]

#[cfg(feature = "mcu-board-support")]
extern crate alloc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub mod ui {
    slint::include_modules!();
}

use slint::*;
use ui::*;

#[cfg(not(feature = "mcu-board-support"))]
mod controllers {
    #[cfg(feature = "chrono")]
    pub mod header;
    #[cfg(feature = "network")]
    pub mod weather;
}
#[cfg(not(feature = "mcu-board-support"))]
use controllers::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let window = MainWindow::new().unwrap();

    // let _ to keep the timer alive.
    #[cfg(all(not(feature = "mcu-board-support"), feature = "chrono"))]
    let _timer = header::setup(&window);

    #[cfg(all(not(feature = "mcu-board-support"), feature = "network"))]
    let weather_join = weather::setup(&window);

    let _kiosk_mode_timer = kiosk_timer(&window);

    window.run().unwrap();

    #[cfg(all(not(feature = "mcu-board-support"), feature = "network"))]
    weather_join.join().unwrap();
}

fn kiosk_timer(window: &MainWindow) -> Timer {
    let kiosk_mode_timer = Timer::default();
    kiosk_mode_timer.start(TimerMode::Repeated, core::time::Duration::from_secs(4), {
        let window_weak = window.as_weak();
        move || {
            if !SettingsAdapter::get(&window_weak.unwrap()).get_kiosk_mode_checked() {
                return;
            }

            let current_page = MenuOverviewAdapter::get(&window_weak.unwrap()).get_current_page();
            let count = MenuOverviewAdapter::get(&window_weak.unwrap()).get_count();

            if current_page >= count - 1 {
                MenuOverviewAdapter::get(&window_weak.unwrap()).set_current_page(0);
            } else {
                MenuOverviewAdapter::get(&window_weak.unwrap()).set_current_page(current_page + 1);
            }
        }
    });

    kiosk_mode_timer
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).unwrap();
    main();
}
