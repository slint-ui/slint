// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore treq TXFF
extern crate alloc;

mod background;
mod display_executor;
mod imu;
mod orientation;
mod tilt_fusion;
pub use imu::{up as ink_up, set_active as set_ink_active};
mod background_job;
mod boot_guard;
pub use background::with_core1;
#[cfg(feature = "pico2-queue-profile")]
mod frame_crc;
mod frame_queue;
mod native_buffer;
#[cfg(feature = "pico2-clock-240")]
mod overclock;
mod packet_slot;
mod panel_scan;
mod panel_sync;
mod pio_transport;
mod scan_budget;
mod scan_phase;
#[cfg(not(feature = "pico2-clock-240"))]
mod stock_clock;
mod uniform_tiles;
mod wire_format;

pub fn panel_tick() -> u32 {
    panel_sync::tick()
}

#[cfg(feature = "pico2-queue-profile")]
mod profile;
#[cfg(feature = "pico2-queue-profile")]
mod profile_run;
mod touch_packet;
mod touch_samples;
mod touch_input;
#[cfg(feature = "pico2-queue-profile")]
mod touch_profile;
#[cfg(feature = "pico2-queue-profile")]
mod touch_replay;
mod usb;

#[cfg(feature = "pico2-queue-profile")]
pub use now_us as profile_now_us;

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_capture_offset() -> Option<u32> {
    (usb::CAPTURE_STATE.load(Ordering::Acquire) != 0)
        .then(|| usb::CAPTURE_OFFSET.load(Ordering::Relaxed))
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_capture_applied() {
    let _ = usb::CAPTURE_STATE.compare_exchange(1, 4, Ordering::AcqRel, Ordering::Relaxed);
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_touch_presses() -> u32 {
    profile::TOUCH_PRESSES.load(Ordering::Acquire)
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_mode() -> u32 {
    profile::RUN.mode()
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_run() -> u32 {
    profile::RUN.token()
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_begin_run(run: u32) {
    profile::begin_run(run);
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_animation_time(us: u64) {
    profile::animation(us);
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_ink_times(times: [u64; 7]) {
    profile::ink_times(times);
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_ink_state(mask: u32, counts: [u32; 4]) {
    let packed = counts.into_iter().enumerate().fold(0, |state, (index, count)| {
        state | ((count | (((mask >> index) & 1) << 7)) << (index * 8))
    });
    profile::INK_STATE.store(packed, Ordering::Release);
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_kiosk_state(enabled: bool, stage: u8) {
    let state = (u32::from(enabled) << 8) | u32::from(stage);
    let previous = profile::KIOSK_STATE.swap(state, Ordering::AcqRel);
    if enabled && previous != state && profile::RUN.mode() == 11 {
        profile::RUN.select(11);
    }
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_dispatch_time(us: u64) {
    profile::dispatch(us);
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_schedule_pointer(x: f32, y: f32, pressed: bool) {
    profile::schedule_pointer(x, y, pressed);
}

#[cfg(feature = "pico2-queue-profile")]
pub fn profile_phase(run: u32, phase: u32) {
    profile::phase(run, phase);
}

use alloc::boxed::Box;
use alloc::vec;
use core::cell::RefCell;
use core::sync::atomic::Ordering;
pub use cortex_m_rt::entry;
use embedded_alloc::LlffHeap as Heap;
use embedded_hal_bus::spi::RefCellDevice;
use static_cell::StaticCell;

use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::pac;
use embassy_rp::spi::Spi;
use embassy_time::Delay;
#[cfg(feature = "panic-probe")]
use panic_probe as _;
use slint::platform::software_renderer::{
    self as renderer, Rgb565BigEndianPixel, SoftwareRenderer,
};
use slint::platform::{PointerEventButton, WindowEvent};

use crate::embassy::{EmbassyBackend, PlatformBackend};

const DISPLAY_SIZE: slint::PhysicalSize = slint::PhysicalSize::new(240, 320);
const SPI_ST7789VW_MAX_FREQ: u32 = 62_500_000;
const HEAP_SIZE: usize = 310 * 1024;

static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

/// The Pixel type of the backing store
pub type TargetPixel = Rgb565BigEndianPixel;

// Keep UART logging independent of embedded-io versions used by the HAL and logger.
struct RawUart0;

// RP2350 UART0 register addresses
const UART0_DR: *mut u32 = 0x4007_0000 as *mut u32;
const UART0_FR: *const u32 = 0x4007_0018 as *const u32;

impl defmt_serial::EraseWrite for RawUart0 {
    fn write(&mut self, buf: &[u8]) {
        for &byte in buf {
            unsafe {
                while core::ptr::read_volatile(UART0_FR) & (1 << 5) != 0 {} // TXFF
                core::ptr::write_volatile(UART0_DR, byte as u32);
            }
        }
    }

    fn flush(&mut self) {
        unsafe {
            while core::ptr::read_volatile(UART0_FR) & (1 << 3) != 0 {} // BUSY
        }
    }
}

pub fn init() {
    boot_guard::recover();
    #[cfg(feature = "pico2-queue-profile")]
    profile::initialize_stack_probe();
    unsafe { ALLOCATOR.init(core::ptr::addr_of_mut!(HEAP) as usize, HEAP_SIZE) }

    #[cfg(not(feature = "pico2-clock-240"))]
    let config = stock_clock::config();
    #[cfg(feature = "pico2-clock-240")]
    let config = overclock::config();
    let p = embassy_rp::init(config);
    boot_guard::arm();
    #[cfg(not(feature = "pico2-clock-240"))]
    stock_clock::configure_flash();
    let backlight = Output::new(p.PIN_16, Level::Low);
    #[cfg(feature = "pico2-renderer-profile")]
    i_slint_renderer_software::embedded_profile::set_clock(profile_now_us);
    usb::start(p.USB, p.CORE1, p.DMA_CH0, backlight);

    #[cfg(feature = "pico2-queue-profile")]
    frame_crc::KNOWN.store(frame_crc::calculate(b"123456789"), Ordering::Release);

    // --- UART0 for defmt_serial (GPIO0 = TX) ---
    // Use embassy UartTx to configure pin mux + baud rate, then forget it.
    {
        let mut uart_config = embassy_rp::uart::Config::default();
        uart_config.baudrate = 115200;
        let uart = embassy_rp::uart::UartTx::new_blocking(p.UART0, p.PIN_0, uart_config);
        core::mem::forget(uart); // keep UART0 configured, don't drop/de-init

        static UART_CELL: StaticCell<RawUart0> = StaticCell::new();
        defmt_serial::defmt_serial(UART_CELL.init(RawUart0));
    }

    // --- SPI1 for display (GPIO10 = SCLK, GPIO11 = MOSI) ---
    let mut spi_config = embassy_rp::spi::Config::default();
    spi_config.frequency = SPI_ST7789VW_MAX_FREQ;

    let spi = Spi::new_blocking_txonly(p.SPI1, p.PIN_10, p.PIN_11, spi_config);

    static SPI_BUS: StaticCell<
        RefCell<Spi<'static, embassy_rp::peripherals::SPI1, embassy_rp::spi::Blocking>>,
    > = StaticCell::new();
    let spi_bus = SPI_BUS.init(RefCell::new(spi));

    // --- Display pins ---
    let dc = Output::new(p.PIN_14, Level::Low);
    let cs = Output::new(p.PIN_13, Level::High);
    let rst = Output::new(p.PIN_15, Level::Low);

    // --- Display init via mipidsi ---
    let display_spi = RefCellDevice::new_no_delay(spi_bus, cs).unwrap();
    static MIPIDSI_BUF: StaticCell<[u8; 512]> = StaticCell::new();
    let mipidsi_buffer = MIPIDSI_BUF.init([0u8; 512]);
    let di = mipidsi::interface::SpiInterface::new(display_spi, dc, mipidsi_buffer);
    let mut display = mipidsi::Builder::new(mipidsi::models::ST7789, di)
        .reset_pin(rst)
        .display_size(DISPLAY_SIZE.width as _, DISPLAY_SIZE.height as _)
        .orientation(mipidsi::options::Orientation::new().rotate(mipidsi::options::Rotation::Deg90))
        .invert_colors(mipidsi::options::ColorInversion::Inverted)
        .init(&mut Delay)
        .unwrap();
    {
        use embedded_graphics::{
            draw_target::DrawTarget,
            pixelcolor::{Rgb565, RgbColor},
        };
        display.clear(Rgb565::BLACK).unwrap();
    }
    frame_queue::configure_scan();
    #[cfg(not(feature = "pico2-clock-240"))]
    pio_transport::init(p.PIO0);

    // --- I2C1 for touch controller (GPIO6 = SDA, GPIO7 = SCL) ---
    let mut i2c_config = embassy_rp::i2c::Config::default();
    i2c_config.frequency = 400_000;
    let i2c = embassy_rp::i2c::I2c::new_blocking(p.I2C1, p.PIN_7, p.PIN_6, i2c_config);

    // --- Reset touch controller (GPIO17) ---
    {
        let mut tp_rst = Output::new(p.PIN_17, Level::High);
        use embedded_hal::delay::DelayNs as _;
        let mut delay = Delay;
        delay.delay_ms(10);
        tp_rst.set_low();
        delay.delay_ms(10);
        tp_rst.set_high();
        delay.delay_ms(100);
        // Keep GPIO17 high (touch not in reset) — don't let drop reconfigure it
        core::mem::forget(tp_rst);
    }

    // --- Touch interrupt pin (GPIO18, active low) ---
    let tp_int = Input::new(p.PIN_18, Pull::Up);

    // --- Touch controller init ---
    let mut touch = cst328::CST328::new(i2c, &mut Delay).unwrap();
    let imu = imu::Imu::new(&mut touch.i2c, now_us());

    touch_input::configure(touch_input::Devices { touch, imu, _pin: tp_int });

    let pico_backend = PicoEmbassyBackend {
        _display: display,
        #[cfg(feature = "pico2-queue-profile")]
        _crc_dma: p.DMA_CH1,
        flipped: false,
        upload_full: false,
        last_touch: None,

        frame_buffer: vec![Rgb565BigEndianPixel::default(); 320 * 240].into_boxed_slice(),
        uniform_tiles: vec![0; uniform_tiles::CACHE_LEN].into_boxed_slice(),
        #[cfg(feature = "pico2-queue-profile")]
        profiler: profile::Profiler::default(),
    };

    // Slint window is landscape (320x240) — swap physical display dimensions
    let window_size = slint::PhysicalSize::new(DISPLAY_SIZE.height, DISPLAY_SIZE.width);
    let embassy_backend =
        EmbassyBackend::new(pico_backend, window_size, renderer::RepaintBufferType::ReusedBuffer);

    frame_queue::DISPLAY_READY.store(true, Ordering::Release);

    slint::platform::set_platform(Box::new(embassy_backend)).expect("backend already initialized");
}

type DisplaySpi = RefCellDevice<
    'static,
    Spi<'static, embassy_rp::peripherals::SPI1, embassy_rp::spi::Blocking>,
    Output<'static>,
    embedded_hal_bus::spi::NoDelay,
>;
type DisplayInterface = mipidsi::interface::SpiInterface<'static, DisplaySpi, Output<'static>>;
type Display = mipidsi::Display<DisplayInterface, mipidsi::models::ST7789, Output<'static>>;

struct PicoEmbassyBackend {
    _display: Display,
    #[cfg(feature = "pico2-queue-profile")]
    _crc_dma: embassy_rp::Peri<'static, embassy_rp::peripherals::DMA_CH1>,
    flipped: bool,
    upload_full: bool,
    last_touch: Option<slint::LogicalPosition>,

    frame_buffer: Box<[Rgb565BigEndianPixel]>,
    uniform_tiles: Box<[u32]>,
    #[cfg(feature = "pico2-queue-profile")]
    profiler: profile::Profiler,
}

impl PlatformBackend for PicoEmbassyBackend {
    const INPUT_BEFORE_RENDER: bool = true;
    async fn dispatch_events(&mut self, window: &slint::Window) {
        boot_guard::heartbeat();
        #[cfg(feature = "pico2-queue-profile")]
        match usb::CAPTURE_STATE.load(Ordering::Acquire) {
            1 | 4 => window.request_redraw(),
            2 => return,
            3 => {
                usb::CAPTURE_STATE.store(0, Ordering::Release);
                window.request_redraw();
            }
            _ => {}
        }
        let wanted = imu::desired_flip();
        if wanted != self.flipped {
            touch_input::cancel(window, &mut self.last_touch);
            self.flipped = wanted;
            touch_input::flip(wanted);
            self.uniform_tiles.reverse();
            self.upload_full = true;
            window.request_redraw();
        }
        #[cfg(feature = "pico2-queue-profile")]
        let dispatch_started = profile::now_us();
        let dispatched = touch_input::dispatch(window, &mut self.last_touch);
        #[cfg(feature = "pico2-queue-profile")]
        {
            let mut dispatched = dispatched;
            if !dispatched && let Some((x, y, kind, sampled)) = profile::take_pointer() {
                profile::sampled_input(sampled);
                if profile_mode() == 2 { profile_phase(profile_run(), panel_tick()); }
                let position = slint::LogicalPosition::new(x, y);
                let button = PointerEventButton::Left;
                let event = match kind {
                    1 => WindowEvent::PointerPressed { position, button },
                    3 => WindowEvent::PointerReleased { position, button },
                    _ => WindowEvent::PointerMoved { position },
                };
                window.dispatch_event(event);
                if kind == 3 { window.dispatch_event(WindowEvent::PointerExited); }
                dispatched = true;
            }
            profile::input(0, profile::now_us() - dispatch_started, dispatched);
        }
        #[cfg(not(feature = "pico2-queue-profile"))]
        let _ = dispatched;
    }

    async fn render(&mut self, renderer: &SoftwareRenderer) {
        #[cfg(feature = "pico2-queue-profile")]
        if usb::CAPTURE_STATE.load(Ordering::Acquire) == 2 {
            return;
        }
        #[cfg(feature = "pico2-queue-profile")]
        let started = profile::now_us();
        #[cfg(feature = "pico2-queue-profile")]
        self.profiler.begin();
        #[cfg(feature = "pico2-queue-profile")]
        let capturing = usb::CAPTURE_STATE.load(Ordering::Acquire) == 4;
        #[cfg(feature = "pico2-queue-profile")]
        let repaint_full = capturing && !usb::CHECK_PANEL.load(Ordering::Relaxed);
        #[cfg(not(feature = "pico2-queue-profile"))]
        let repaint_full = false;
        if repaint_full {
            renderer.set_repaint_buffer_type(renderer::RepaintBufferType::NewBuffer);
        }
        #[cfg(feature = "pico2-queue-profile")]
        profile::begin_raster();
        let region = renderer.render_into_buffer(&mut native_buffer::FrameBuffer::new(
            &mut self.frame_buffer,
            320,
        ));
        #[cfg(feature = "pico2-queue-profile")]
        let raster_done = profile::now_us();
        #[cfg(feature = "pico2-queue-profile")]
        let frame_hash = {
            let bytes = unsafe {
                core::slice::from_raw_parts(
                    self.frame_buffer.as_ptr().cast::<u8>(),
                    self.frame_buffer.len() * 2,
                )
            };
            frame_crc::calculate(bytes)
        };
        #[cfg(not(feature = "pico2-queue-profile"))]
        let frame_hash = 0;
        #[cfg(feature = "pico2-queue-profile")]
        let mut stats = profile::FrameStats {
            raster_us: raster_done - started,
            crc_us: profile::now_us() - raster_done,
            ..Default::default()
        };

        {
            let publishing = frame_queue::publish(
                &self.frame_buffer,
                &region,
                frame_hash,
                &mut self.uniform_tiles,
                self.flipped,
                self.upload_full,
            );
            let adapter = i_slint_core::renderer::RendererSealed::window_adapter(renderer);
            let _transfer = if let Some(adapter) = adapter {
                touch_input::while_publishing(publishing, adapter.window(), &mut self.last_touch).await
            } else {
                publishing.await
            };
            #[cfg(feature = "pico2-queue-profile")]
            if capturing {
                frame_queue::wait_delivered(_transfer.frame_id).await;
            }
            #[cfg(feature = "pico2-queue-profile")]
            {
                stats.wait_us = _transfer.wait_us;
                stats.window_us = _transfer.copy_us;
                stats.plan_us = _transfer.plan_us;
                stats.pixels = _transfer.pixels;
                stats.lines = _transfer.lines;
            }
        }
        if repaint_full {
            renderer.set_repaint_buffer_type(renderer::RepaintBufferType::ReusedBuffer);
        }
        self.upload_full = false;
        #[cfg(feature = "pico2-queue-profile")]
        if capturing {
            self.profiler = profile::Profiler::default();
            let pixels = self.frame_buffer.as_mut_ptr().cast();
            usb::CAPTURE_PIXELS.store(pixels, Ordering::Release);
            usb::CAPTURE_STATE.store(2, Ordering::Release);
        } else {
            self.profiler.frame(profile::now_us() - started, &stats);
        }
    }

    async fn wait_for_event(&mut self) {
        while !touch_input::ready() {
            boot_guard::heartbeat();
            embassy_time::Timer::after_micros(500).await;
        }
    }
}

/// Pin masks for SIO GPIO control (GPIO13=CS, GPIO14=DC)
const CS_PIN_MASK: u32 = 1 << 13;
const DC_PIN_MASK: u32 = 1 << 14;

fn flush_spi1() {
    while pac::SPI1.sr().read().bsy() {}
}

fn drain_spi1_rx() {
    while pac::SPI1.sr().read().rne() {
        let _ = pac::SPI1.dr().read();
    }
}

fn cs_low() {
    pac::SIO.gpio_out(0).value_clr().write_value(CS_PIN_MASK);
}

fn cs_high() {
    pac::SIO.gpio_out(0).value_set().write_value(CS_PIN_MASK);
}

fn dc_high() {
    pac::SIO.gpio_out(0).value_set().write_value(DC_PIN_MASK);
}
mod cst328 {
    use embedded_hal::i2c::I2c;

    const TP_ADDR: u8 = 0x1A;

    // CST328 register commands (high byte = register page, low byte = value/offset)
    const CMD_DEBUG_INFO_MODE: [u8; 2] = [0xD1, 0x01];
    const CMD_READ_CHIP_ID: [u8; 2] = [0xD1, 0xFC];
    const CMD_NORMAL_MODE: [u8; 2] = [0xD1, 0x09];
    const CMD_READ_TOUCH_DATA: [u8; 2] = [0xD0, 0x00];

    pub struct CST328<I2C: I2c> {
        pub(super) i2c: I2C,
        tracker: super::touch_packet::Tracker,
    }

    impl<I2C: I2c> CST328<I2C> {
        pub fn new(
            mut i2c: I2C,
            delay: &mut impl embedded_hal::delay::DelayNs,
        ) -> Result<Self, I2C::Error> {
            // Enter debug info mode
            i2c.write(TP_ADDR, &CMD_DEBUG_INFO_MODE)?;
            delay.delay_ms(10);

            // Read chip ID (not used, but part of init sequence)
            i2c.write(TP_ADDR, &CMD_READ_CHIP_ID)?;
            let mut chip_id = [0u8; 4];
            i2c.read(TP_ADDR, &mut chip_id)?;

            // Enter normal mode
            i2c.write(TP_ADDR, &CMD_NORMAL_MODE)?;
            delay.delay_ms(10);
            i2c.write(TP_ADDR, &[0xd0, 0x00, 0xab])?;

            Ok(Self { i2c, tracker: Default::default() })
        }

        pub fn read(&mut self) -> Result<super::touch_packet::Report, I2C::Error> {
            let mut data = [0u8; super::touch_packet::REPORT_BYTES];
            self.i2c.write_read(TP_ADDR, &CMD_READ_TOUCH_DATA, &mut data)?;
            let report = self.tracker.read(&data);
            #[cfg(feature = "pico2-queue-profile")]
            super::touch_profile::report(&data, report);
            self.i2c.write(TP_ADDR, &[0xd0, 0x05, 0])?;
            self.i2c.write(TP_ADDR, &[0xd0, 0x00, 0xab])?;
            Ok(report)
        }
    }
}

pub fn now_us() -> u64 {
    embassy_time::Instant::now().as_micros()
}
