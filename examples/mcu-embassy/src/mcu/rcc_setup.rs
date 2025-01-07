// Copyright © 2025 David Haig
// SPDX-License-Identifier: MIT

use embassy_stm32::time::Hertz;
use embassy_stm32::{rcc, Config, Peripherals};

/// Sets up clocks for the stm32u5g9zj mcu
/// change this if you plan to use a different microcontroller
pub fn stm32u5g9zj_init() -> Peripherals {
    // setup power and clocks for an STM32U5G9J-DK2 run from an external 16 Mhz external oscillator
    let mut config = Config::default();
    config.rcc.hse = Some(rcc::Hse { freq: Hertz(16_000_000), mode: rcc::HseMode::Oscillator });
    config.rcc.pll1 = Some(rcc::Pll {
        source: rcc::PllSource::HSE,
        prediv: rcc::PllPreDiv::DIV1,
        mul: rcc::PllMul::MUL10,
        divp: None,
        divq: None,
        divr: Some(rcc::PllDiv::DIV1),
    });
    config.rcc.sys = rcc::Sysclk::PLL1_R; // 160 Mhz
    config.rcc.pll3 = Some(rcc::Pll {
        source: rcc::PllSource::HSE,
        prediv: rcc::PllPreDiv::DIV4, // PLL_M
        mul: rcc::PllMul::MUL125,     // PLL_N
        divp: None,
        divq: None,
        divr: Some(rcc::PllDiv::DIV20),
    });
    config.rcc.mux.ltdcsel = rcc::mux::Ltdcsel::PLL3_R; // 25 MHz
    embassy_stm32::init(config)
}
