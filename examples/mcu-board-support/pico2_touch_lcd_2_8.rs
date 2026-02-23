// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use core::cell::RefCell;
use core::sync::atomic::{Ordering, compiler_fence};
pub use cortex_m_rt::entry;
use embedded_alloc::LlffHeap as Heap;
use embedded_hal_bus::spi::RefCellDevice;
use static_cell::StaticCell;

use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::pac;
use embassy_rp::pac::dma::vals::{DataSize, TreqSel};
use embassy_rp::spi::Spi;
use embassy_time::Delay;
#[cfg(feature = "panic-probe")]
use panic_probe as _;
use slint::platform::software_renderer::{self as renderer, Rgb565Pixel, SoftwareRenderer};
use slint::platform::{PointerEventButton, WindowEvent};

use crate::embassy::{EmbassyBackend, PlatformBackend};

const DISPLAY_SIZE: slint::PhysicalSize = slint::PhysicalSize::new(240, 320);
const SPI_ST7789VW_MAX_FREQ: u32 = 62_500_000;
const HEAP_SIZE: usize = 400 * 1024;

static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

/// The Pixel type of the backing store
pub type TargetPixel = Rgb565Pixel;

/// Raw UART0 writer for defmt_serial.
/// Embassy-rp 0.4.0 implements embedded_io 0.6, but defmt_serial 0.11.0 requires 0.7.
/// Bypass the version mismatch by writing to UART0 registers directly.
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
    unsafe { ALLOCATOR.init(core::ptr::addr_of_mut!(HEAP) as usize, HEAP_SIZE) }

    // embassy-rp defaults to 150 MHz PLL_SYS for RP235x with 12 MHz crystal
    let config = embassy_rp::config::Config::default();
    let p = embassy_rp::init(config);

    // --- UART0 for defmt_serial (GPIO0 = TX) ---
    // Use embassy UartTx to configure pin mux + baud rate, then forget it.
    // defmt_serial needs embedded_io 0.7 Write, but embassy-rp 0.4.0 implements
    // embedded_io 0.6 — so we use a raw register wrapper instead.
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
    let backlight = Output::new(p.PIN_16, Level::High);
    // Embassy-rp Output::drop() sets pin function to NULL, which would turn off
    // the backlight when init() returns. Forget it to keep the pin configured.
    core::mem::forget(backlight);

    // --- Display init via mipidsi ---
    let display_spi = RefCellDevice::new_no_delay(spi_bus, cs).unwrap();
    static MIPIDSI_BUF: StaticCell<[u8; 512]> = StaticCell::new();
    let mipidsi_buffer = MIPIDSI_BUF.init([0u8; 512]);
    let di = mipidsi::interface::SpiInterface::new(display_spi, dc, mipidsi_buffer);
    let display = mipidsi::Builder::new(mipidsi::models::ST7789, di)
        .reset_pin(rst)
        .display_size(DISPLAY_SIZE.width as _, DISPLAY_SIZE.height as _)
        .invert_colors(mipidsi::options::ColorInversion::Inverted)
        .init(&mut Delay)
        .unwrap();

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
    let touch = cst328::CST328::new(i2c, &mut Delay).unwrap();

    // --- Line buffers (double-buffered for DMA) ---
    let line_buffer_a =
        vec![Rgb565Pixel::default(); DISPLAY_SIZE.width as usize].into_boxed_slice();
    let line_buffer_b =
        vec![Rgb565Pixel::default(); DISPLAY_SIZE.width as usize].into_boxed_slice();

    let pico_backend = PicoEmbassyBackend {
        display,
        touch,
        last_touch: None,
        line_buffer_a,
        line_buffer_b,
        tp_int,
    };

    // Slint window is landscape (320x240) — swap physical display dimensions
    let window_size = slint::PhysicalSize::new(DISPLAY_SIZE.height, DISPLAY_SIZE.width);
    let embassy_backend =
        EmbassyBackend::new(pico_backend, window_size, renderer::RepaintBufferType::ReusedBuffer);

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
    display: Display,
    touch: cst328::CST328<
        embassy_rp::i2c::I2c<'static, embassy_rp::peripherals::I2C1, embassy_rp::i2c::Blocking>,
    >,
    last_touch: Option<slint::LogicalPosition>,
    line_buffer_a: Box<[Rgb565Pixel]>,
    line_buffer_b: Box<[Rgb565Pixel]>,
    tp_int: Input<'static>,
}

impl PlatformBackend for PicoEmbassyBackend {
    async fn dispatch_events(&mut self, window: &slint::Window) {
        let button = PointerEventButton::Left;
        if let Some(event) = self
            .touch
            .read()
            .map_err(|_| ())
            .unwrap()
            .map(|point| {
                // Touch reports physical portrait coords (x: 0..240, y: 0..320).
                // Map to logical landscape window (320x240) for Rotate90 rendering:
                //   logical_x = raw_y,  logical_y = 240 - raw_x
                let position = slint::PhysicalPosition::new(
                    point.y as _,
                    (DISPLAY_SIZE.width as f32 - point.x) as _,
                )
                .to_logical(window.scale_factor());
                match self.last_touch.replace(position) {
                    Some(_) => WindowEvent::PointerMoved { position },
                    None => WindowEvent::PointerPressed { position, button },
                }
            })
            .or_else(|| {
                self.last_touch
                    .take()
                    .map(|position| WindowEvent::PointerReleased { position, button })
            })
        {
            let is_pointer_release_event = matches!(event, WindowEvent::PointerReleased { .. });

            window.dispatch_event(event);

            if is_pointer_release_event {
                window.dispatch_event(WindowEvent::PointerExited);
            }
        }
    }

    async fn render(&mut self, renderer: &SoftwareRenderer) {
        renderer.set_rendering_rotation(renderer::RenderingRotation::Rotate90);
        let mut provider = DmaLineBufferProvider {
            display: &mut self.display,
            buffer: &mut self.line_buffer_a,
            dma_buffer: &mut self.line_buffer_b,
            dma_busy: false,
        };
        renderer.render_by_line(&mut provider);
        provider.flush_frame();
    }

    async fn wait_for_event(&mut self) {
        self.tp_int.wait_for_low().await;
    }
}

// --- DMA helper functions ---
// These program DMA channel 0 directly via PAC registers, mirroring
// embassy-rp's own dma.rs copy_inner (lines 130-168).

/// Pin masks for SIO GPIO control (GPIO13=CS, GPIO14=DC)
const CS_PIN_MASK: u32 = 1 << 13;
const DC_PIN_MASK: u32 = 1 << 14;

fn start_dma_to_spi1(src: *const u8, byte_count: usize) {
    let ch = pac::DMA.ch(0);
    ch.read_addr().write_value(src as u32);
    // SPI1 DR register address (SPI1 base 0x4008_8000 + DR offset 0x08)
    ch.write_addr().write_value(0x4008_8008);
    ch.trans_count().write(|w| {
        w.set_mode(0.into()); // NORMAL: transfer once then stop
        w.set_count(byte_count as u32);
    });
    compiler_fence(Ordering::SeqCst);
    ch.ctrl_trig().write(|w| {
        w.set_treq_sel(TreqSel::SPI1_TX);
        w.set_data_size(DataSize::SIZE_BYTE);
        w.set_incr_read(true);
        w.set_incr_write(false);
        w.set_chain_to(0); // self-chain = no chaining
        w.set_en(true);
    });
    compiler_fence(Ordering::SeqCst);
}

fn wait_dma() {
    while pac::DMA.ch(0).ctrl_trig().read().busy() {}
}

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

// --- DMA double-buffered line provider ---

struct DmaLineBufferProvider<'a> {
    display: &'a mut Display,
    buffer: &'a mut [Rgb565Pixel],
    dma_buffer: &'a mut [Rgb565Pixel],
    dma_busy: bool,
}

impl DmaLineBufferProvider<'_> {
    fn flush_frame(&mut self) {
        if self.dma_busy {
            wait_dma();
            flush_spi1();
            drain_spi1_rx();
            cs_high();
            self.dma_busy = false;
        }
    }
}

impl renderer::LineBufferProvider for &mut DmaLineBufferProvider<'_> {
    type TargetPixel = Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Rgb565Pixel]),
    ) {
        // 1. Render pixels into CPU buffer
        render_fn(&mut self.buffer[range.clone()]);

        // 2. Byte-swap LE→BE for ST7789
        for x in &mut self.buffer[range.clone()] {
            *x = Rgb565Pixel(x.0.to_be());
        }

        // 3. If DMA is busy from previous line, wait for it
        if self.dma_busy {
            wait_dma();
            flush_spi1();
            cs_high();
        }

        // 4. Swap buffers — CPU-rendered data moves to DMA buffer
        core::mem::swap(&mut self.buffer, &mut self.dma_buffer);

        // 5. Send window command via mipidsi (empty pixel iterator = command only)
        self.display
            .set_pixels(
                range.start as u16,
                line as _,
                range.end as u16,
                line as u16,
                core::iter::empty(),
            )
            .unwrap();

        // 6. Assert CS, set DC=data, start DMA from dma_buffer
        cs_low();
        dc_high();
        let byte_count = range.len() * core::mem::size_of::<Rgb565Pixel>();
        let src = self.dma_buffer[range.start..range.end].as_ptr() as *const u8;
        start_dma_to_spi1(src, byte_count);
        self.dma_busy = true;
    }
}

mod cst328 {
    use embedded_hal::i2c::I2c;
    use euclid::default::Point2D;

    const TP_ADDR: u8 = 0x1A;

    // CST328 register commands (high byte = register page, low byte = value/offset)
    const CMD_DEBUG_INFO_MODE: [u8; 2] = [0xD1, 0x01];
    const CMD_READ_CHIP_ID: [u8; 2] = [0xD1, 0xFC];
    const CMD_NORMAL_MODE: [u8; 2] = [0xD1, 0x09];
    const CMD_READ_TOUCH_DATA: [u8; 2] = [0xD0, 0x00];

    pub struct CST328<I2C: I2c> {
        i2c: I2C,
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

            Ok(Self { i2c })
        }

        pub fn read(&mut self) -> Result<Option<Point2D<f32>>, I2C::Error> {
            self.i2c.write(TP_ADDR, &CMD_READ_TOUCH_DATA)?;
            let mut data = [0u8; 27];
            self.i2c.read(TP_ADDR, &mut data)?;

            let finger_state = data[0] & 0x0F;
            if finger_state == 6 {
                let raw_x = ((data[1] as u16) << 4) | ((data[3] as u16) >> 4);
                let raw_y = ((data[2] as u16) << 4) | (data[3] as u16 & 0x0F);

                let x = raw_x as f32;
                let y = raw_y as f32;

                Ok(Some(euclid::point2(x, y)))
            } else {
                Ok(None)
            }
        }
    }
}
