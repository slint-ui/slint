// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The safety UI on an ESP32-S3-BOX-3, bare metal on esp-hal and embassy.
//!
//! [`board::init`] brings up the panel and the touch controller, the esp-rtos
//! scheduler provides embassy's time driver, and the UI runs as the main
//! embassy task through [`slint_safeui_app::app_main`].

#![no_std]
#![no_main]

extern crate alloc;

mod board;
mod platform;

use esp_backtrace as _;

esp_bootloader_esp_idf::esp_app_desc!();

/// `esp_rtos::main` wraps this in the thread-mode embassy executor. It only
/// returns on [`slint_safeui_app::AppEvent::Quit`], which this backend never
/// reports.
#[esp_rtos::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let board = board::init();

    // Start the scheduler that backs embassy's time driver.
    esp_rtos::start(board.timer, board.software_interrupt);

    slint_safeui_app::app_main(board.platform).await;
}
