// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// https://github.com/raspberrypi/pico-examples/tree/master/pio/st7789_lcd
use core::sync::atomic::{AtomicU32, Ordering};
use embassy_rp::pac;

static ACTIVE: AtomicU32 = AtomicU32::new(0);

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed) != 0
}

pub fn hz() -> u32 {
    let active = ACTIVE.load(Ordering::Relaxed);
    if active != 0 {
        return active;
    }
    let spi = pac::SPI1;
    embassy_rp::clocks::clk_peri_freq()
        / (u32::from(spi.cpsr().read().cpsdvsr()) * (u32::from(spi.cr0().read().scr()) + 1))
}

pub fn init(_pio: embassy_rp::Peri<'static, embassy_rp::peripherals::PIO0>) {
    super::flush_spi1();
    pac::RESETS.reset().modify(|v| v.set_pio0(false));
    while !pac::RESETS.reset_done().read().pio0() {}
    let pio = pac::PIO0;
    pio.ctrl().modify(|v| v.set_sm_enable(0));
    // OUT PINS, 1 SIDE 0; MOV Y, Y SIDE 1. Wrap after the second instruction.
    pio.instr_mem(0).write(|v| v.set_instr_mem(0x6001));
    pio.instr_mem(1).write(|v| v.set_instr_mem(0xb042));
    let sm = pio.sm(0);
    let divider =
        (u64::from(embassy_rp::clocks::clk_sys_freq()) * 128).div_ceil(60_000_000u64) as u32;
    sm.clkdiv().write(|v| {
        v.set_int((divider >> 8) as u16);
        v.set_frac(divider as u8);
    });
    sm.execctrl().write(|v| {
        v.set_wrap_bottom(0);
        v.set_wrap_top(1);
    });
    sm.pinctrl().write(|v| {
        v.set_out_base(11);
        v.set_out_count(1);
        v.set_sideset_base(10);
        v.set_sideset_count(1);
        v.set_set_base(10);
        v.set_set_count(2);
    });
    sm.shiftctrl().write(|v| {
        v.set_autopull(true);
        v.set_pull_thresh(8);
        v.set_out_shiftdir(false);
    });
    sm.shiftctrl().modify(|v| v.set_fjoin_tx(true));
    sm.instr().write(|v| v.set_instr(0xe083));
    sm.instr().write(|v| v.set_instr(0xe000));
    for pin in [10, 11] {
        pac::IO_BANK0
            .gpio(pin)
            .ctrl()
            .modify(|v| v.set_funcsel(6.into()));
    }
    pio.ctrl().modify(|v| {
        v.set_sm_restart(1);
        v.set_clkdiv_restart(1);
    });
    sm.instr().write(|v| v.set_instr(0));
    let actual = (u64::from(embassy_rp::clocks::clk_sys_freq()) * 128 / u64::from(divider)) as u32;
    ACTIVE.store(actual, Ordering::Release);
    pio.ctrl().modify(|v| v.set_sm_enable(1));
}

#[unsafe(link_section = ".data.ram_func")]
#[inline(never)]
pub fn flush() {
    // TXSTALL recurs on OUT while the FIFO is empty, after the final clock edge.
    pac::PIO0
        .fdebug()
        .write_value(pac::pio::regs::Fdebug(1 << 24));
    while pac::PIO0.fdebug().read().0 & (1 << 24) == 0 {}
}
#[unsafe(link_section = ".data.ram_func")]
#[inline(never)]
pub fn bits(count: u8) {
    flush();
    pac::PIO0.ctrl().modify(|v| v.set_sm_enable(0));
    pac::PIO0
        .sm(0)
        .shiftctrl()
        .modify(|v| v.set_pull_thresh(count));
    pac::PIO0.ctrl().modify(|v| v.set_sm_restart(1));
    pac::PIO0.sm(0).instr().write(|v| v.set_instr(0));
    pac::PIO0.ctrl().modify(|v| v.set_sm_enable(1));
}
#[inline(always)]
pub fn write(value: u32) {
    while pac::PIO0.fstat().read().0 & (1 << 16) != 0 {}
    pac::PIO0.txf(0).write_value(value);
}
pub fn target() -> u32 {
    if active() {
        pac::PIO0.txf(0).as_ptr() as u32
    } else {
        pac::SPI1.dr().as_ptr() as u32
    }
}
pub fn dreq() -> pac::dma::vals::TreqSel {
    if active() {
        pac::dma::vals::TreqSel::PIO0_TX0
    } else {
        pac::dma::vals::TreqSel::SPI1_TX
    }
}
