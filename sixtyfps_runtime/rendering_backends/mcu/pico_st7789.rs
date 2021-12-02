// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use embedded_time::rate::*;
use rp_pico::hal;
use rp_pico::hal::pac;
use rp_pico::hal::prelude::*;

use embedded_hal::digital::v2::OutputPin;

pub use cortex_m_rt::entry;

use defmt_rtt as _; // global logger
use panic_probe as _;

#[alloc_error_handler]
fn oom(_: core::alloc::Layout) -> ! {
    loop {}
}
use alloc_cortex_m::CortexMHeap;

const HEAP_SIZE: usize = 128 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

pub fn init_board() {
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    let mut watchdog = hal::watchdog::Watchdog::new(pac.WATCHDOG);

    let clocks = hal::clocks::init_clocks_and_plls(
        rp_pico::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().integer());

    let sio = hal::sio::Sio::new(pac.SIO);

    let pins = rp_pico::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);

    let _spi_sclk = pins.gpio10.into_mode::<hal::gpio::FunctionSpi>();
    let _spi_mosi = pins.gpio11.into_mode::<hal::gpio::FunctionSpi>();
    let _spi_miso = pins.gpio12.into_mode::<hal::gpio::FunctionSpi>();

    let spi = hal::spi::Spi::<_, _, 8>::new(pac.SPI1);

    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        4_000_000u32.Hz(),
        &embedded_hal::spi::MODE_3,
    );

    let rst = pins.gpio15.into_push_pull_output();

    let dc = pins.gpio8.into_push_pull_output();
    let cs = pins.gpio9.into_push_pull_output();
    let di = display_interface_spi::SPIInterface::new(spi, dc, cs);

    let mut display = st7789::ST7789::new(di, rst, 320, 240);

    // Turn on backlight
    {
        let mut bl = pins.gpio13.into_push_pull_output();
        bl.set_low().unwrap();
        delay.delay_us(10_000);
        bl.set_high().unwrap();
    }

    unsafe { ALLOCATOR.init(&mut HEAP as *const u8 as usize, core::mem::size_of_val(&HEAP)) }

    display.init(&mut delay).unwrap();
    display.set_orientation(st7789::Orientation::Landscape).unwrap();

    crate::init_with_display(display);
}
