// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use embassy_rp::pac;

const TRIAL_MARKER: u32 = 0x50494332;

pub fn config() -> embassy_rp::config::Config {
    if pac::WATCHDOG.scratch0().read() == TRIAL_MARKER {
        return embassy_rp::config::Config::default();
    }
    // RP2350 QMI requires an uncached dummy read after increasing its divisor.
    pac::QMI.mem(0).timing().modify(|timing| {
        if timing.clkdiv() != 0 && timing.clkdiv() < 4 {
            timing.set_clkdiv(4);
        }
    });
    cortex_m::asm::dsb();
    unsafe { core::ptr::read_volatile(0x1400_0000 as *const u32) };
    cortex_m::asm::dsb();
    cortex_m::asm::isb();
    pac::WATCHDOG.scratch0().write_value(TRIAL_MARKER);
    embassy_rp::config::Config::new(
        embassy_rp::clocks::ClockConfig::system_freq(240_000_000).unwrap(),
    )
}
