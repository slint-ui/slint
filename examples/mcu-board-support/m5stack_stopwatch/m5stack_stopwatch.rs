// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore Cst CST datasheet Ioe IOE QSPI qspi RAMWR RAMWRC sio

//! Board support for the M5Stack StopWatch Dev Kit (ESP32-S3).
//!
//! The kit pairs an ESP32-S3R8 with a round 1.75" 466x466 AMOLED panel driven
//! by a CO5300 over QSPI, plus a CST820 capacitive touch controller on I2C.
//!
//! Panel power, panel reset and touch reset are not wired to the ESP32. They
//! hang off an M5IOE1 I2C GPIO expander, so the expander has to come up first
//! or the panel stays dark and the touch controller never answers.

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use embedded_hal_bus::i2c::RefCellDevice;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::Blocking;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::dma_buffers;
use esp_hal::i2c::master::{BusTimeout, Config as I2cConfig, I2c};
use esp_hal::peripherals::Peripherals;
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Address, Command, Config as SpiConfig, DataMode, Spi, SpiDmaBus};
use esp_hal::time::{Instant, Rate};
use esp_println::logger::init_logger_from_env;
use log::{error, info};
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{LogicalPosition, PhysicalPosition, PhysicalSize};
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();

const DISPLAY_WIDTH: u16 = 466;
const DISPLAY_HEIGHT: u16 = 466;

/// The CO5300 addresses a 468 pixel wide window, and the visible area of this
/// panel starts at column 6.
const X_OFFSET: u16 = 6;
const Y_OFFSET: u16 = 0;

/// One DMA transfer's worth of pixel data. Matches the buffer allocated below.
const DMA_CHUNK_SIZE: usize = 16380;

// -------------------------------------------------------------------------
// M5IOE1 I2C GPIO expander
// -------------------------------------------------------------------------

// Only the low byte of each register is used here, because every pin this
// board support needs sits on P1-P8.
const IOE_REG_UID_L: u8 = 0x00;
/// How many times to retry a transfer to the expander. M5Stack's own driver
/// retries too, because the part does not always answer first time.
const IOE_RETRIES: usize = 4;

/// How long the expander needs between the register pointer write and the
/// read that follows it.
const REGISTER_SETTLE_US: u32 = 1000;
const IOE_REG_I2C_CFG: u8 = 0x23;
const IOE_REG_GPIO_MODE_L: u8 = 0x03;
const IOE_REG_GPIO_OUT_L: u8 = 0x05;
const IOE_REG_GPIO_IN_L: u8 = 0x07;
const IOE_REG_GPIO_PU_L: u8 = 0x09;
const IOE_REG_GPIO_PD_L: u8 = 0x0b;
const IOE_REG_GPIO_DRV_L: u8 = 0x13;

// The datasheet counts pins from one, the registers count bits from zero.
// These are bit positions, so P1 is 0.
const IOE_PIN_MUX_CTRL: u8 = 0; // P1: CH442E multiplexer
const IOE_PIN_AUDIO_EN: u8 = 2; // P3: audio rail
const IOE_PIN_TOUCH_RST: u8 = 3; // P4: CST820 reset
const IOE_PIN_PANEL_RST: u8 = 4; // P5: CO5300 reset
const IOE_PIN_PANEL_POWER: u8 = 7; // P8: L3B rail that feeds the AMOLED

/// Minimal M5IOE1 driver: enough to drive the rails and reset lines.
struct M5Ioe1<BUS> {
    i2c: BUS,
    address: u8,
    delay: Delay,
}

impl<BUS: embedded_hal::i2c::I2c> M5Ioe1<BUS> {
    /// The expander answers on 0x4f on this board. M5Stack's own driver falls
    /// back to 0x6f, which is the part's default address, so do the same.
    ///
    /// The expander sleeps between transfers and the transfer that wakes it is
    /// itself NACKed, so every address is tried a few times. M5Stack's driver
    /// does the same thing: it retries twice with a 50ms gap, and sends its
    /// first two configuration writes back to back.
    fn new(mut i2c: BUS, delay: &mut Delay) -> Result<Self, BUS::Error> {
        let mut last_error = None;
        for address in [0x4f, 0x6f] {
            for _ in 0..4 {
                let mut uid = [0u8; 1];
                let probe = i2c.write(address, &[IOE_REG_UID_L]).and_then(|()| {
                    // The expander needs a moment to latch the register
                    // pointer; reading too soon is NACKed.
                    delay.delay_micros(REGISTER_SETTLE_US);
                    i2c.read(address, &mut uid)
                });
                match probe {
                    Ok(()) => {
                        info!("M5IOE1 found at address {:#04x}", address);
                        let mut expander = Self { i2c, address, delay: *delay };
                        // Keep it awake from here on, so that later register
                        // writes do not have to fight the same wake-up NACK.
                        expander.disable_sleep(delay);
                        return Ok(expander);
                    }
                    Err(e) => last_error = Some(e),
                }
                delay.delay_millis(50);
            }
        }
        Err(last_error.unwrap())
    }

    /// Clear the sleep time in the I2C configuration register, the same way
    /// M5Stack's setI2cSleepTime(0) does. Best effort: it is retried, and a
    /// failure here is not fatal.
    fn disable_sleep(&mut self, delay: &mut Delay) {
        for _ in 0..4 {
            if let Ok(config) = self.read_register(IOE_REG_I2C_CFG) {
                let awake = config & 0xf0;
                if self.write_register(IOE_REG_I2C_CFG, awake).is_ok() {
                    return;
                }
            }
            delay.delay_millis(50);
        }
        error!("Could not stop the M5IOE1 from sleeping");
    }

    /// Set the register pointer and read it back as two separate transfers.
    /// The expander NACKs the repeated start that `write_read` would use.
    fn read_register(&mut self, register: u8) -> Result<u8, BUS::Error> {
        let mut last_error = None;
        for _ in 0..IOE_RETRIES {
            let mut value = [0u8; 1];
            let result = self.i2c.write(self.address, &[register]).and_then(|()| {
                self.delay.delay_micros(REGISTER_SETTLE_US);
                self.i2c.read(self.address, &mut value)
            });
            match result {
                Ok(()) => return Ok(value[0]),
                Err(e) => last_error = Some(e),
            }
            self.delay.delay_millis(5);
        }
        Err(last_error.unwrap())
    }

    fn write_register(&mut self, register: u8, value: u8) -> Result<(), BUS::Error> {
        let mut last_error = None;
        for _ in 0..IOE_RETRIES {
            match self.i2c.write(self.address, &[register, value]) {
                Ok(()) => {
                    self.delay.delay_micros(REGISTER_SETTLE_US);
                    return Ok(());
                }
                Err(e) => last_error = Some(e),
            }
            self.delay.delay_millis(5);
        }
        Err(last_error.unwrap())
    }

    fn update_bit(&mut self, register: u8, bit: u8, set: bool) -> Result<(), BUS::Error> {
        let mut value = self.read_register(register)?;
        if set {
            value |= 1 << bit;
        } else {
            value &= !(1 << bit);
        }
        self.write_register(register, value)
    }

    /// Same effect as M5IOE1::pinMode(pin, OUTPUT): drive mode on, pulls off,
    /// push-pull rather than open drain.
    fn set_output(&mut self, pin: u8) -> Result<(), BUS::Error> {
        self.update_bit(IOE_REG_GPIO_MODE_L, pin, true)?;
        self.update_bit(IOE_REG_GPIO_PU_L, pin, false)?;
        self.update_bit(IOE_REG_GPIO_PD_L, pin, false)?;
        self.update_bit(IOE_REG_GPIO_DRV_L, pin, false)
    }

    fn write_pin(&mut self, pin: u8, level: bool) -> Result<(), BUS::Error> {
        self.update_bit(IOE_REG_GPIO_OUT_L, pin, level)
    }

    fn read_pin(&mut self, pin: u8) -> Result<bool, BUS::Error> {
        Ok(self.read_register(IOE_REG_GPIO_IN_L)? & (1 << pin) != 0)
    }
}

/// List every device that acknowledges its address, so a missing expander can
/// be told apart from a bus that is not working at all.
fn scan_i2c_bus<BUS: embedded_hal::i2c::I2c>(i2c: &mut BUS) {
    let mut found = 0;
    for address in 0x08..=0x77u8 {
        let mut byte = [0u8; 1];
        let readable = i2c.read(address, &mut byte).is_ok();
        // Pointing at register 0 is harmless on every device on this bus.
        let writable = i2c.write(address, &[0x00]).is_ok();
        if readable || writable {
            info!("i2c: {:#04x} read={} write={}", address, readable, writable);
            found += 1;
        }
    }
    info!("i2c: {} device(s) responded", found);
}

/// Switch on the rails the panel and the touch controller need.
///
/// The AMOLED rail is the one that matters: M5Stack's firmware writes it and
/// then reads it back until it reads high. Without that retry the panel
/// sometimes never powers up.
fn power_up<BUS: embedded_hal::i2c::I2c>(
    ioe: &mut M5Ioe1<BUS>,
    delay: &mut Delay,
) -> Result<(), BUS::Error> {
    for pin in [
        IOE_PIN_MUX_CTRL,
        IOE_PIN_AUDIO_EN,
        IOE_PIN_TOUCH_RST,
        IOE_PIN_PANEL_RST,
        IOE_PIN_PANEL_POWER,
    ] {
        ioe.set_output(pin)?;
    }

    ioe.write_pin(IOE_PIN_MUX_CTRL, false)?;
    ioe.write_pin(IOE_PIN_AUDIO_EN, true)?;
    ioe.write_pin(IOE_PIN_TOUCH_RST, true)?;
    ioe.write_pin(IOE_PIN_PANEL_RST, true)?;
    ioe.write_pin(IOE_PIN_PANEL_POWER, true)?;

    for attempt in 0..10 {
        delay.delay_millis(80);
        if ioe.read_pin(IOE_PIN_PANEL_POWER)? {
            return Ok(());
        }
        info!("AMOLED rail still low, retry {}", attempt + 1);
        ioe.write_pin(IOE_PIN_PANEL_POWER, true)?;
    }

    error!("AMOLED rail never read back high");
    Ok(())
}

fn reset_panel<BUS: embedded_hal::i2c::I2c>(
    ioe: &mut M5Ioe1<BUS>,
    delay: &mut Delay,
) -> Result<(), BUS::Error> {
    ioe.write_pin(IOE_PIN_PANEL_RST, false)?;
    delay.delay_millis(20);
    ioe.write_pin(IOE_PIN_PANEL_RST, true)?;
    delay.delay_millis(150);
    Ok(())
}

fn reset_touch<BUS: embedded_hal::i2c::I2c>(
    ioe: &mut M5Ioe1<BUS>,
    delay: &mut Delay,
) -> Result<(), BUS::Error> {
    ioe.write_pin(IOE_PIN_TOUCH_RST, false)?;
    delay.delay_millis(10);
    ioe.write_pin(IOE_PIN_TOUCH_RST, true)?;
    delay.delay_millis(50);
    Ok(())
}

// -------------------------------------------------------------------------
// CO5300 AMOLED controller on QSPI
// -------------------------------------------------------------------------

/// Commands and their parameters go out on a single data line, wrapped in
/// opcode 0x02 with the command byte in the address phase.
const QSPI_CONTROL_OPCODE: u16 = 0x02;
/// Pixel data goes out on all four data lines, wrapped in opcode 0x32.
const QSPI_PIXEL_OPCODE: u16 = 0x32;
const CMD_RAMWR: u32 = 0x2c;
const CMD_RAMWRC: u32 = 0x3c;

struct Co5300 {
    spi: SpiDmaBus<'static, Blocking>,
}

impl Co5300 {
    fn command(&mut self, command: u8, parameters: &[u8]) -> Result<(), esp_hal::spi::Error> {
        self.spi.half_duplex_write(
            DataMode::Single,
            Command::_8Bit(QSPI_CONTROL_OPCODE, DataMode::Single),
            Address::_24Bit((command as u32) << 8, DataMode::Single),
            0,
            parameters,
        )
    }

    /// Initialization sequence taken from M5Stack's Panel_CO5300.
    fn init(&mut self, delay: &mut Delay) -> Result<(), esp_hal::spi::Error> {
        self.command(0x11, &[])?; // sleep out
        delay.delay_millis(120);
        self.command(0x34, &[0x00])?; // tearing effect off
        self.command(0xfe, &[0x00])?; // switch to the user command page
        self.command(0xc4, &[0x80])?; // QSPI mode
        self.command(0x3a, &[0x55])?; // 16 bits per pixel
        self.command(0x36, &[0x00])?; // memory access control
        self.command(0x53, &[0x20])?; // brightness control on
        self.command(0x63, &[0xff])?; // brightness in high brightness mode
        self.command(0x29, &[])?; // display on
        self.command(0x51, &[0xff])?; // brightness in normal mode
        self.command(0x58, &[0x00])?; // high contrast mode off
        Ok(())
    }

    fn set_window(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), esp_hal::spi::Error> {
        let x_start = x + X_OFFSET;
        let x_end = x_start + width - 1;
        self.command(
            0x2a,
            &[(x_start >> 8) as u8, x_start as u8, (x_end >> 8) as u8, x_end as u8],
        )?;

        let y_start = y + Y_OFFSET;
        let y_end = y_start + height - 1;
        self.command(0x2b, &[(y_start >> 8) as u8, y_start as u8, (y_end >> 8) as u8, y_end as u8])
    }

    /// Send one rectangle of the frame buffer to the panel.
    ///
    /// The CO5300 takes RGB565 most significant byte first, so the pixels are
    /// byte swapped into `scratch` on the way out. `scratch` also keeps the
    /// data in internal RAM, which the DMA reaches faster than PSRAM.
    fn write_region(
        &mut self,
        frame_buffer: &[Rgb565Pixel],
        stride: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        scratch: &mut [u8],
    ) -> Result<(), esp_hal::spi::Error> {
        if width == 0 || height == 0 {
            return Ok(());
        }

        // The CO5300 needs the write window on even column and row
        // boundaries; an odd start or extent shifts the landing position
        // and leaves stale-pixel trails behind shrinking elements. Widen
        // the rectangle outward to even alignment - the frame buffer holds
        // the whole frame, so the extra pixels are always valid.
        let x_aligned = x & !1;
        let y_aligned = y & !1;
        let width = ((width + (x - x_aligned) + 1) & !1).min(DISPLAY_WIDTH as u16 - x_aligned);
        let height = ((height + (y - y_aligned) + 1) & !1).min(DISPLAY_HEIGHT as u16 - y_aligned);
        let (x, y) = (x_aligned, y_aligned);

        self.set_window(x, y, width, height)?;

        let rows_per_chunk = (scratch.len() / (width as usize * 2)).max(1);
        let mut first = true;

        let mut row = y;
        while row < y + height {
            let rows = rows_per_chunk.min((y + height - row) as usize);

            let mut used = 0;
            for line in 0..rows {
                let start = (row as usize + line) * stride + x as usize;
                let pixels = &frame_buffer[start..start + width as usize];
                for pixel in pixels {
                    scratch[used..used + 2].copy_from_slice(&pixel.0.to_be_bytes());
                    used += 2;
                }
            }

            let address = if first { CMD_RAMWR } else { CMD_RAMWRC } << 8;
            self.spi.half_duplex_write(
                DataMode::Quad,
                Command::_8Bit(QSPI_PIXEL_OPCODE, DataMode::Single),
                Address::_24Bit(address, DataMode::Single),
                0,
                &scratch[..used],
            )?;

            first = false;
            row += rows as u16;
        }

        Ok(())
    }
}

// -------------------------------------------------------------------------
// CST820 touch controller
// -------------------------------------------------------------------------

const CST820_ADDRESS: u8 = 0x15;

struct Cst820<BUS> {
    i2c: BUS,
}

impl<BUS: embedded_hal::i2c::I2c> Cst820<BUS> {
    fn new(i2c: BUS) -> Self {
        Self { i2c }
    }

    /// Returns the touch position, or None while no finger is down.
    fn read(&mut self) -> Result<Option<(u16, u16)>, BUS::Error> {
        let mut buffer = [0u8; 7];
        self.i2c.write_read(CST820_ADDRESS, &[0x00], &mut buffer)?;

        let fingers = buffer[2];
        // The top two bits of the X high byte carry the event: 0 down,
        // 1 up, 2 contact.
        let event = buffer[3] >> 6;
        if fingers == 0 || event == 1 {
            return Ok(None);
        }

        let x = (((buffer[3] & 0x0f) as u16) << 8) | buffer[4] as u16;
        let y = (((buffer[5] & 0x0f) as u16) << 8) | buffer[6] as u16;
        Ok(Some((x, y)))
    }
}

// -------------------------------------------------------------------------
// Slint platform
// -------------------------------------------------------------------------

/// The board's I2C bus, shared with whatever else hangs off it: the audio
/// codec, the IMU, the RTC and the power management chip.
static mut SHARED_I2C: Option<&'static RefCell<I2c<'static, Blocking>>> = None;

/// Whether the panel should be lit. The application flips this (for
/// example, to dark the screen while audio is idle); the event loop sends
/// the display on/off command when the value changes.
static DISPLAY_ON: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(true);

/// Ask for the display to be on or off. Takes effect within one event-loop
/// iteration; rendering is skipped while the panel is off.
pub fn set_display_on(on: bool) {
    DISPLAY_ON.store(on, core::sync::atomic::Ordering::Relaxed);
}

/// Run `f` with the board's I2C bus, once the event loop has set it up.
/// Returns None before then.
pub fn with_i2c<R>(f: impl FnOnce(&mut I2c<'static, Blocking>) -> R) -> Option<R> {
    let bus = unsafe { *core::ptr::addr_of!(SHARED_I2C) }?;
    let mut bus = bus.borrow_mut();
    Some(f(&mut bus))
}

struct EspBackend {
    window: RefCell<Option<Rc<MinimalSoftwareWindow>>>,
    peripherals: RefCell<Option<Peripherals>>,
}

impl Default for EspBackend {
    fn default() -> Self {
        EspBackend { window: RefCell::new(None), peripherals: RefCell::new(None) }
    }
}

impl slint::platform::Platform for EspBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
        window.set_size(PhysicalSize::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32));
        self.window.replace(Some(window.clone()));
        Ok(window)
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(Instant::now().duration_since_epoch().as_millis())
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        self.run_event_loop()
    }
}

/// Initializes the heap in PSRAM and sets the Slint platform.
pub fn init() {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_160MHz));

    init_logger_from_env();

    // The frame buffer alone is 466 * 466 * 2 bytes, so the heap lives in the
    // 8 MB of PSRAM rather than in internal RAM.
    esp_alloc::psram_allocator!(unsafe { esp_hal::peripherals::PSRAM::steal() }, esp_hal::psram);

    slint::platform::set_platform(Box::new(EspBackend {
        peripherals: RefCell::new(Some(peripherals)),
        window: RefCell::new(None),
    }))
    .expect("backend already initialized");
}

impl EspBackend {
    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let peripherals = self.peripherals.borrow_mut().take().expect("Peripherals already taken");
        let mut delay = Delay::new();

        // Every I2C device on this board shares one bus: the expander, the
        // touch controller, the PMU, the IMU and the RTC.
        let i2c = I2c::new(
            peripherals.I2C0,
            I2cConfig::default()
                .with_frequency(Rate::from_khz(100))
                .with_timeout(BusTimeout::Maximum),
        )
        .map_err(|e| slint::PlatformError::Other(alloc::format!("Failed to set up I2C: {e:?}")))?
        .with_sda(peripherals.GPIO47)
        .with_scl(peripherals.GPIO48);

        static I2C_BUS: StaticCell<RefCell<I2c<'static, Blocking>>> = StaticCell::new();
        let i2c_bus = I2C_BUS.init(RefCell::new(i2c));
        // Publish the bus so an application can reach the other devices on it.
        unsafe { SHARED_I2C = Some(i2c_bus) };

        let mut ioe = match M5Ioe1::new(RefCellDevice::new(i2c_bus), &mut delay) {
            Ok(ioe) => ioe,
            Err(e) => {
                scan_i2c_bus(&mut RefCellDevice::new(i2c_bus));
                return Err(slint::PlatformError::Other(alloc::format!(
                    "No M5IOE1 GPIO expander: {e:?}"
                )));
            }
        };

        power_up(&mut ioe, &mut delay).map_err(|e| {
            slint::PlatformError::Other(alloc::format!("Failed to power up the board: {e:?}"))
        })?;
        reset_panel(&mut ioe, &mut delay).map_err(|e| {
            slint::PlatformError::Other(alloc::format!("Failed to reset the panel: {e:?}"))
        })?;
        reset_touch(&mut ioe, &mut delay).map_err(|e| {
            slint::PlatformError::Other(alloc::format!("Failed to reset the touch panel: {e:?}"))
        })?;

        let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(DMA_CHUNK_SIZE);
        let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
        let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

        let spi = Spi::new(
            peripherals.SPI2,
            SpiConfig::default().with_frequency(Rate::from_mhz(40_u32)).with_mode(Mode::_0),
        )
        .map_err(|e| slint::PlatformError::Other(alloc::format!("Failed to set up SPI: {e:?}")))?
        .with_sio0(peripherals.GPIO41)
        .with_sio1(peripherals.GPIO42)
        .with_sio2(peripherals.GPIO46)
        .with_sio3(peripherals.GPIO45)
        .with_cs(peripherals.GPIO39)
        .with_sck(peripherals.GPIO40)
        .with_dma(peripherals.DMA_CH0)
        .with_buffers(dma_rx_buf, dma_tx_buf);

        let mut display = Co5300 { spi };
        let mut display_lit = true;
        display.init(&mut delay).map_err(|e| {
            slint::PlatformError::Other(alloc::format!("Failed to initialize the panel: {e:?}"))
        })?;
        info!("CO5300 initialized");

        let mut touch = Cst820::new(RefCellDevice::new(i2c_bus));

        // The frame buffer lives in PSRAM; the scratch buffer the pixels are
        // byte swapped through stays in internal RAM.
        let mut frame_buffer =
            alloc::vec![Rgb565Pixel(0); DISPLAY_WIDTH as usize * DISPLAY_HEIGHT as usize];
        static PIXEL_SCRATCH: StaticCell<[u8; DMA_CHUNK_SIZE]> = StaticCell::new();
        let scratch = PIXEL_SCRATCH.init([0u8; DMA_CHUNK_SIZE]);

        let mut last_touch: Option<LogicalPosition> = None;

        info!("Entering the event loop");

        loop {
            slint::platform::update_timers_and_animations();

            let Some(window) = self.window.borrow().clone() else {
                continue;
            };

            match touch.read() {
                Ok(Some((x, y))) => {
                    let position =
                        PhysicalPosition::new(x as i32, y as i32).to_logical(window.scale_factor());
                    let event = match last_touch.replace(position) {
                        Some(previous) if previous == position => None,
                        Some(_) => Some(WindowEvent::PointerMoved { position }),
                        None => Some(WindowEvent::PointerPressed {
                            position,
                            button: PointerEventButton::Left,
                        }),
                    };
                    if let Some(event) = event {
                        window.try_dispatch_event(event)?;
                    }
                }
                Ok(None) => {
                    if let Some(position) = last_touch.take() {
                        window.try_dispatch_event(WindowEvent::PointerReleased {
                            position,
                            button: PointerEventButton::Left,
                        })?;
                        window.try_dispatch_event(WindowEvent::PointerExited)?;
                    }
                }
                Err(e) => error!("Touch read failed: {e:?}"),
            }

            let want_on = DISPLAY_ON.load(core::sync::atomic::Ordering::Relaxed);
            if want_on != display_lit {
                display_lit = want_on;
                if let Err(e) = display.command(if want_on { 0x29 } else { 0x28 }, &[]) {
                    error!("Display power command failed: {e:?}");
                }
            }
            if !display_lit {
                // The panel shows nothing; skip rendering entirely and
                // repaint everything when it comes back.
                if !window.has_active_animations() {
                    delay.delay_millis(10);
                }
                continue;
            }
            window.draw_if_needed(|renderer| {
                let region = renderer.render(&mut frame_buffer, DISPLAY_WIDTH as usize);
                for (origin, size) in region.iter() {
                    if let Err(e) = display.write_region(
                        &frame_buffer,
                        DISPLAY_WIDTH as usize,
                        origin.x as u16,
                        origin.y as u16,
                        size.width as u16,
                        size.height as u16,
                        scratch,
                    ) {
                        error!("Failed to send pixels to the panel: {e:?}");
                    }
                }
            });

            if !window.has_active_animations() {
                delay.delay_millis(10);
            }
        }
    }
}
