// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cSpell: ignore TIMG

//! Bringing up the ESP32-S3-BOX-3: the PSRAM heap, the ILI9342C panel on SPI2,
//! and the GT911 touch controller on I2C0.

use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{DriveMode, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::i2c::master::I2c;
use esp_hal::interrupt::software::{SoftwareInterrupt, SoftwareInterruptControl};
use esp_hal::spi::Mode as SpiMode;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::Rate;
use esp_hal::timer::timg::{Timer, TimerGroup};
use gt911::Gt911Blocking;
use log::{error, info, warn};
use mipidsi::options::{ColorOrder, Orientation, Rotation};
use static_cell::StaticCell;

use crate::platform::Esp32Platform;

/// The panel, 320x240 pixels driven by an ILI9342C over SPI.
pub const PANEL_WIDTH: u32 = 320;
pub const PANEL_HEIGHT: u32 = 240;

/// The I2C address the GT911 is asked to take, and the one it falls back to.
const GT911_ADDRESS: u8 = 0x14;
const GT911_ADDRESS_BACKUP: u8 = 0x5d;

pub type BoardI2c = I2c<'static, esp_hal::Blocking>;
pub type BoardDisplay = mipidsi::Display<
    mipidsi::interface::SpiInterface<
        'static,
        ExclusiveDevice<Spi<'static, esp_hal::Blocking>, Output<'static>, Delay>,
        Output<'static>,
    >,
    mipidsi::models::ILI9342CRgb565,
    Output<'static>,
>;

/// Everything [`init`] hands back: the UI backend, plus the timer and software
/// interrupt the esp-rtos scheduler needs to drive embassy.
pub struct Board {
    pub platform: Esp32Platform,
    pub timer: Timer<'static>,
    pub software_interrupt: SoftwareInterrupt<'static, 0>,
}

/// Initialize the chip, the PSRAM heap and the board peripherals.
pub fn init() -> Board {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_240MHz));
    esp_println::logger::init_logger_from_env();
    info!("Peripherals initialized");

    // Register the PSRAM heap before anything allocates: the frame buffer the
    // scene renders into is far larger than the internal RAM can spare.
    esp_alloc::psram_allocator!(
        peripherals.PSRAM,
        esp_hal::psram,
        esp_hal::psram::PsramConfig {
            mode: esp_hal::psram::PsramMode::OctalSpi,
            ..Default::default()
        }
    );

    let mut delay = Delay::new();

    // The GT911 picks its I2C address from the INT pin level while RESET is
    // released, and on this board RESET is shared with the display. Drive the
    // sequence before either driver comes up.
    // See https://github.com/espressif/esp-bsp/issues/302#issuecomment-1971559689
    let mut int_pin = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    int_pin.set_low();
    delay.delay_ms(10);

    let mut reset = Output::new(
        peripherals.GPIO48,
        Level::Low,
        OutputConfig::default().with_drive_mode(DriveMode::OpenDrain),
    );
    reset.set_low();
    delay.delay_ms(10);

    // INT low selects 0x14, INT high would select the backup address 0x5d.
    int_pin.set_low();
    delay.delay_ms(1);
    reset.set_high();
    delay.delay_ms(60);

    let spi = Spi::<esp_hal::Blocking>::new(
        peripherals.SPI2,
        SpiConfig::default().with_frequency(Rate::from_mhz(40)).with_mode(SpiMode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO7)
    .with_mosi(peripherals.GPIO6);

    let dc = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
    let cs = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());

    // The interface buffer has to outlive the display, so it lives in a `StaticCell`.
    // It bounds how much of a frame goes out per SPI transaction, and the backend
    // pushes whole frames, so it's worth more than the single line mipidsi needs.
    const SPI_BUFFER_SIZE: usize = 4096;
    let spi_device = ExclusiveDevice::new(spi, cs, delay).unwrap();
    static SPI_BUFFER: StaticCell<[u8; SPI_BUFFER_SIZE]> = StaticCell::new();
    let interface = mipidsi::interface::SpiInterface::new(
        spi_device,
        dc,
        SPI_BUFFER.init([0u8; SPI_BUFFER_SIZE]),
    );

    let display = mipidsi::Builder::new(mipidsi::models::ILI9342CRgb565, interface)
        .reset_pin(reset)
        .orientation(Orientation::new().rotate(Rotation::Deg180))
        .color_order(ColorOrder::Bgr)
        .init(&mut delay)
        .unwrap();
    info!("Display initialized");

    // Bringing the display up pulsed the shared reset line, so the controller
    // read the INT level again; only now is it done with it. Hand the line back,
    // which is how it announces that a report is waiting.
    let mut touch_int = int_pin.into_flex();
    touch_int.set_output_enable(false);
    touch_int.apply_input_config(&InputConfig::default().with_pull(Pull::Up));
    touch_int.set_input_enable(true);

    let mut backlight = Output::new(peripherals.GPIO47, Level::Low, OutputConfig::default());
    backlight.set_high();

    let mut i2c = I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO8)
    .with_scl(peripherals.GPIO18);

    let mut touch = Gt911Blocking::new(GT911_ADDRESS);
    if let Err(e) = touch.init(&mut i2c) {
        warn!("Touch initialization failed: {:?}, trying the backup address", e);
        touch = Gt911Blocking::new(GT911_ADDRESS_BACKUP);
        if let Err(e) = touch.init(&mut i2c) {
            error!("Touch initialization failed with the backup address too: {:?}", e);
        }
    }

    let timer_group = TimerGroup::new(peripherals.TIMG0);
    let software_interrupts = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    Board {
        platform: Esp32Platform::new(display, touch, i2c, backlight, touch_int),
        timer: timer_group.timer0,
        software_interrupt: software_interrupts.software_interrupt0,
    }
}
