// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};
use embassy_rp::pac;

#[cfg(not(feature = "pico2-clock-240"))]
const LIVE_MARKER: u32 = 0x50494331;
const TIMEOUT_US: u32 = 8_000_000;
static HEARTBEAT_MS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
pub static TEST_STALL: AtomicU32 = AtomicU32::new(0);

pub fn recover() {
    #[cfg(not(feature = "pico2-clock-240"))]
    if pac::WATCHDOG.scratch1().read() == LIVE_MARKER && pac::WATCHDOG.reason().read().timer() {
        pac::WATCHDOG.ctrl().modify(|w| w.set_enable(false));
        permit_next_boot();
        embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    }
}

pub fn permit_next_boot() {
    pac::WATCHDOG.scratch1().write_value(0);
    pac::WATCHDOG.scratch0().write_value(0);
}

pub fn arm() {
    #[cfg(feature = "pico2-clock-240")]
    if embassy_rp::clocks::clk_sys_freq() != 240_000_000 {
        return;
    }
    #[cfg(not(feature = "pico2-clock-240"))]
    pac::WATCHDOG.scratch1().write_value(LIVE_MARKER);
    pac::WATCHDOG.ctrl().modify(|w| w.set_enable(false));
    pac::PSM.wdsel().write_value(pac::psm::regs::Wdsel(0x0001ffff & !3));
    pac::WATCHDOG.load().write_value(pac::watchdog::regs::Load(TIMEOUT_US));
    pac::WATCHDOG.ctrl().modify(|w| w.set_enable(true));
    heartbeat();
}

pub fn heartbeat() {
    #[cfg(feature = "pico2-queue-profile")]
    if TEST_STALL.load(Ordering::Acquire) == 1 {
        loop {
            core::hint::spin_loop();
        }
    }
    HEARTBEAT_MS.store(embassy_time::Instant::now().as_millis() as u32, Ordering::Release);
}

#[embassy_executor::task]
pub async fn watchdog_task() {
    loop {
        #[cfg(feature = "pico2-queue-profile")]
        if TEST_STALL.load(Ordering::Acquire) == 2 {
            loop {
                core::hint::spin_loop();
            }
        }
        let now = embassy_time::Instant::now().as_millis() as u32;
        if now.wrapping_sub(HEARTBEAT_MS.load(Ordering::Acquire)) < 4_000 {
            pac::WATCHDOG.load().write_value(pac::watchdog::regs::Load(TIMEOUT_US));
        }
        embassy_time::Timer::after_millis(250).await;
    }
}
