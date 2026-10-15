// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore rcmd rfmt

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

#[unsafe(link_section = ".data.ram_func")]
#[inline(never)]
pub fn configure_flash() {
    let memory = embassy_rp::pac::QMI.mem(0);
    // Basic 03h reads are limited to 50 MHz on the W25Q128JV.
    if embassy_rp::clocks::clk_sys_freq() == 150_000_000
        && memory.rcmd().read().prefix() == 0xeb
        && memory.rfmt().read().data_width() == embassy_rp::pac::qmi::vals::DataWidth::Q
    {
        // Non-divider timing fields require an idle QMI; core 1 has not started yet.
        cortex_m::interrupt::free(|_| {
            let controller = embassy_rp::pac::QMI;
            controller.direct_csr().modify(|control| control.set_en(true));
            while controller.direct_csr().read().busy() {}
            memory.timing().modify(|timing| {
                timing.set_clkdiv(2);
                timing.set_min_deselect(1);
            });
            cortex_m::asm::dsb();
            controller.direct_csr().modify(|control| control.set_en(false));
            cortex_m::asm::dsb();
            cortex_m::asm::isb();
        });
    }
}
