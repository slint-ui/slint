// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]

use crate::app_main::AppHandler;

pub mod ui {
    slint::include_modules!();
}

mod app_main;
mod weather;

fn main() -> Result<(), slint::PlatformError> {
    env_logger::Builder::default()
        .filter_level(if cfg!(debug_assertions) {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .init();

    let mut app_handler = AppHandler::new();
    app_handler.initialize_ui();

    let res = app_handler.run();
    app_handler.save();
    res
}
