// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub fn config() -> embassy_rp::config::Config {
    use embassy_rp::clocks::{ClockConfig, PeriClkSrc};
    let mut clocks = ClockConfig::crystal(12_000_000);
    let pll = clocks.xosc.as_mut().unwrap().usb_pll.as_mut().unwrap();
    // 12 MHz × 120 / 5 / 3 = 96 MHz; USB and ADC divide this back to 48 MHz.
    pll.post_div1 = 5;
    pll.post_div2 = 3;
    clocks.usb_clk.as_mut().unwrap().div = 2;
    clocks.adc_clk.as_mut().unwrap().div = 2;
    clocks.peri_clk_src = Some(PeriClkSrc::PllUsb);
    embassy_rp::config::Config::new(clocks)
}
