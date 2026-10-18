// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use embassy_rp::pac;

const SCK: u32 = 1 << 10;
const MOSI: u32 = 1 << 11;

// Preserve the qualified delay in time when the CPU changes frequency.
#[inline(always)]
fn read_delay() {
    #[cfg(feature = "pico2-clock-240")]
    unsafe {
        core::arch::asm!(".rept 48", "nop", ".endr", options(nomem, nostack, preserves_flags))
    };
    #[cfg(not(feature = "pico2-clock-240"))]
    unsafe {
        core::arch::asm!(".rept 30", "nop", ".endr", options(nomem, nostack, preserves_flags))
    };
}

#[unsafe(link_section = ".data.ram_func")]
#[inline(never)]
fn clock(input: bool, release_data: bool) -> bool {
    read_delay();
    pac::SIO.gpio_out(0).value_set().write_value(SCK);
    read_delay();
    if release_data {
        pac::SIO.gpio_oe(0).value_clr().write_value(MOSI);
    }
    let value = input && pac::SIO.gpio_in(0).read() & MOSI != 0;
    pac::SIO.gpio_out(0).value_clr().write_value(SCK);
    value
}

#[inline(always)]
fn begin_read(command: u8) {
    super::cs_low();
    pac::SIO.gpio_out(0).value_clr().write_value(super::DC_PIN_MASK);
    for bit in (0..8).rev() {
        if command & (1 << bit) != 0 {
            pac::SIO.gpio_out(0).value_set().write_value(MOSI);
        } else {
            pac::SIO.gpio_out(0).value_clr().write_value(MOSI);
        }
        clock(false, bit == 0);
    }
    super::dc_high();
    pac::SIO.gpio_out(0).value_clr().write_value(MOSI);
    pac::SIO.gpio_oe(0).value_clr().write_value(MOSI);
}

#[inline(always)]
fn end_read() {
    super::cs_high();
    pac::SIO.gpio_oe(0).value_set().write_value(MOSI);
}

#[unsafe(link_section = ".data.ram_func")]
#[inline(never)]
fn read(command: u8) -> u32 {
    begin_read(command);
    // ST7789 returns one dummy bit before the 16 scanline bits.
    let mut raw = 0;
    for _ in 0..24 {
        raw = (raw << 1) | u32::from(clock(true, false));
    }
    end_read();
    raw
}

#[inline(always)]
fn with_read_pins<R>(read: impl FnOnce() -> R) -> R {
    if super::pio_transport::active() {
        super::pio_transport::flush();
    } else {
        super::flush_spi1();
    }
    let sck_control = pac::IO_BANK0.gpio(10).ctrl().read();
    let mosi_control = pac::IO_BANK0.gpio(11).ctrl().read();
    let pad = pac::PADS_BANK0.gpio(11).read();
    pac::PADS_BANK0.gpio(11).modify(|w| w.set_ie(true));
    pac::IO_BANK0.gpio(10).ctrl().modify(|w| w.set_funcsel(5.into()));
    pac::IO_BANK0.gpio(11).ctrl().modify(|w| w.set_funcsel(5.into()));
    pac::SIO.gpio_oe(0).value_set().write_value(SCK | MOSI);
    pac::SIO.gpio_out(0).value_clr().write_value(SCK);
    let value = read();
    pac::IO_BANK0.gpio(10).ctrl().write_value(sck_control);
    pac::IO_BANK0.gpio(11).ctrl().write_value(mosi_control);
    pac::PADS_BANK0.gpio(11).write_value(pad);
    value
}

#[unsafe(link_section = ".data.ram_func")]
#[inline(never)]
pub fn scanline() -> Option<u16> {
    let scan = ((with_read_pins(|| read(0x45)) >> 7) & 0x1ff) as u16;
    (scan < 350).then_some(scan)
}

/// Read controller memory for diagnostics while the USB task holds the renderer.
#[cfg(feature = "pico2-queue-profile")]
pub fn memory_row(pixels: &mut [u16]) {
    with_read_pins(|| {
        begin_read(0x2e);
        // RAMRD has an eight-clock dummy byte, unlike the scan register read.
        for _ in 0..8 {
            clock(true, false);
        }
        for pixel in pixels {
            let mut rgb = [0u8; 3];
            for byte in &mut rgb {
                for _ in 0..8 {
                    *byte = (*byte << 1) | u8::from(clock(true, false));
                }
            }
            *pixel = (u16::from(rgb[0] >> 3) << 11)
                | (u16::from(rgb[1] >> 2) << 5)
                | u16::from(rgb[2] >> 3);
        }
        end_read();
    });
}
