// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use core::cell::RefCell;
use core::convert::Infallible;
use cortex_m::interrupt::Mutex;
use cortex_m::singleton;
pub use cortex_m_rt::entry;
use embedded_alloc::LlffHeap as Heap;
use static_cell::StaticCell;

use embedded_hal::digital::OutputPin;
use embedded_hal::spi::{ErrorType, Operation, SpiBus, SpiDevice};
use fugit::{Hertz, RateExtU32};
use hal::dma::{DMAExt, SingleChannel, WriteTarget};
use hal::timer::{Alarm, Alarm0};
use pac::interrupt;
#[cfg(feature = "panic-probe")]
use panic_probe as _;
use renderer::Rgb565Pixel;

use hal::{Timer, pac, prelude::*, timer::CopyableTimer0};
use rp235x_hal as hal;
use slint::platform::{PointerEventButton, WindowEvent, software_renderer as renderer};

#[unsafe(link_section = ".start_block")]
#[unsafe(no_mangle)]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

const XOSC_CRYSTAL_FREQ: u32 = 12_000_000;

const HEAP_SIZE: usize = 400 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

static ALARM0: Mutex<RefCell<Option<Alarm0<CopyableTimer0>>>> = Mutex::new(RefCell::new(None));
static TIMER: Mutex<RefCell<Option<Timer<CopyableTimer0>>>> = Mutex::new(RefCell::new(None));

type TpIntPin = gpio::Pin<gpio::bank0::Gpio18, gpio::FunctionSio<gpio::SioInput>, gpio::PullUp>;
static TP_INT_PIN: Mutex<RefCell<Option<TpIntPin>>> = Mutex::new(RefCell::new(None));

/// Wrapper around PAC UART0 for use with defmt_serial.
struct PacUart(pac::UART0);

impl defmt_serial::EraseWrite for PacUart {
    fn write(&mut self, buf: &[u8]) {
        for &byte in buf {
            while self.0.uartfr().read().txff().bit_is_set() {}
            self.0.uartdr().write(|w| unsafe { w.data().bits(byte) });
        }
    }

    fn flush(&mut self) {
        while self.0.uartfr().read().busy().bit_is_set() {}
    }
}

// 16ns for serial clock cycle (write), page 43 of https://www.waveshare.com/w/upload/a/ae/ST7789_Datasheet.pdf
const SPI_ST7789VW_MAX_FREQ: Hertz<u32> = Hertz::<u32>::Hz(62_500_000);

const DISPLAY_SIZE: slint::PhysicalSize = slint::PhysicalSize::new(320, 240);

/// The Pixel type of the backing store
pub type TargetPixel = Rgb565Pixel;

type SpiPins = (
    gpio::Pin<gpio::bank0::Gpio11, gpio::FunctionSpi, gpio::PullDown>,
    gpio::Pin<gpio::bank0::Gpio10, gpio::FunctionSpi, gpio::PullDown>,
);

type EnabledSpi = hal::Spi<hal::spi::Enabled, pac::SPI1, SpiPins, 8>;
type SpiRefCell = RefCell<(EnabledSpi, Hertz<u32>)>;
type Display<DI, RST> = mipidsi::Display<DI, mipidsi::models::ST7789, RST>;

use hal::gpio::{self, Interrupt as GpioInterrupt};

#[derive(Clone)]
struct SharedSpiWithFreq<CS> {
    refcell: &'static SpiRefCell,
    cs: CS,
    freq: Hertz<u32>,
    peri_freq: Hertz<u32>,
}

impl<CS> ErrorType for SharedSpiWithFreq<CS> {
    type Error = <EnabledSpi as ErrorType>::Error;
}

impl<CS: OutputPin<Error = Infallible>> SpiDevice for SharedSpiWithFreq<CS> {
    #[inline]
    fn transaction(&mut self, operations: &mut [Operation<u8>]) -> Result<(), Self::Error> {
        let mut borrowed = self.refcell.borrow_mut();
        if borrowed.1 != self.freq {
            borrowed.0.flush()?;
            borrowed.0.set_baudrate(self.peri_freq, self.freq);
            borrowed.1 = self.freq;
        }
        self.cs.set_low()?;
        for op in operations {
            match op {
                Operation::Read(words) => borrowed.0.read(words),
                Operation::Write(words) => borrowed.0.write(words),
                Operation::Transfer(read, write) => borrowed.0.transfer(read, write),
                Operation::TransferInPlace(words) => borrowed.0.transfer_in_place(words),
                Operation::DelayNs(_) => unimplemented!(),
            }?;
        }
        borrowed.0.flush()?;
        drop(borrowed);
        self.cs.set_high()?;
        Ok(())
    }
}

pub fn init() {
    let mut pac = pac::Peripherals::take().unwrap();

    // === Normal clock init (PLL_SYS = 150 MHz, peripheral clock = 150 MHz) ===
    let mut watchdog = hal::watchdog::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    unsafe { ALLOCATOR.init(core::ptr::addr_of_mut!(HEAP) as usize, HEAP_SIZE) }

    let mut timer = hal::Timer::new_timer0(pac.TIMER0, &mut pac.RESETS, &clocks);

    let sio = hal::sio::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);

    let _tx_pin = pins.gpio0.into_function::<hal::gpio::FunctionUart>();
    pac.RESETS.reset().modify(|_, w| w.uart0().clear_bit());
    while pac.RESETS.reset_done().read().uart0().bit_is_clear() {}
    let peri_freq = clocks.peripheral_clock.freq().to_Hz();
    let baud_rate_div = (4 * peri_freq as u64 / 115200) as u32;
    let uart0 = pac.UART0;
    uart0.uartibrd().write(|w| unsafe { w.bits(baud_rate_div >> 6) });
    uart0.uartfbrd().write(|w| unsafe { w.bits(baud_rate_div & 0x3F) });
    uart0.uartlcr_h().write(|w| unsafe { w.wlen().bits(0b11).fen().set_bit() });
    uart0.uartcr().write(|w| w.uarten().set_bit().txe().set_bit());
    static UART_CELL: StaticCell<PacUart> = StaticCell::new();
    defmt_serial::defmt_serial(UART_CELL.init(PacUart(uart0)));

    // Pins::new resets IO_BANK0, so GPIO0 UART function is wiped — reconfigure it

    let rst = pins.gpio15.into_push_pull_output();
    let mut backlight = pins.gpio16.into_push_pull_output();
    backlight.set_high().unwrap();

    let dc = pins.gpio14.into_push_pull_output();
    let mut cs = pins.gpio13.into_push_pull_output();

    let spi_sclk = pins.gpio10.into_function::<gpio::FunctionSpi>();
    let spi_mosi = pins.gpio11.into_function::<gpio::FunctionSpi>();

    let spi: EnabledSpi = hal::Spi::new(pac.SPI1, (spi_mosi, spi_sclk)).init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        SPI_ST7789VW_MAX_FREQ,
        &embedded_hal::spi::MODE_0,
    );

    // SAFETY: This is not safe :-(  But we need to access the SPI and its control pins for the PIO
    let (dc_copy, cs_copy) =
        unsafe { (core::ptr::read(&dc as *const _), core::ptr::read(&cs as *const _)) };
    let stolen_spi = unsafe { core::ptr::read(&spi as *const _) };

    let spi = singleton!(:SpiRefCell = SpiRefCell::new((spi, 0.Hz()))).unwrap();
    let mipidsi_buffer = singleton!(:[u8; 512] = [0; 512]).unwrap();

    cs.set_high().unwrap(); // CS must be deasserted before display init/reset
    let peri_freq = clocks.peripheral_clock.freq();
    let display_spi =
        SharedSpiWithFreq { refcell: spi, cs, freq: SPI_ST7789VW_MAX_FREQ, peri_freq };
    let di = mipidsi::interface::SpiInterface::new(display_spi, dc, mipidsi_buffer);
    let display = mipidsi::Builder::new(mipidsi::models::ST7789, di)
        .reset_pin(rst)
        .display_size(DISPLAY_SIZE.height as _, DISPLAY_SIZE.width as _)
        .orientation(mipidsi::options::Orientation::new().rotate(mipidsi::options::Rotation::Deg90))
        .invert_colors(mipidsi::options::ColorInversion::Inverted)
        .init(&mut timer)
        .unwrap();

    // --- I2C1 for touch controller ---
    let sda = pins.gpio6.reconfigure();
    let scl = pins.gpio7.reconfigure();
    let i2c =
        hal::I2C::i2c1(pac.I2C1, sda, scl, 400.kHz(), &mut pac.RESETS, clocks.system_clock.freq());

    // --- Reset touch controller (GPIO17) ---
    let mut tp_rst = pins.gpio17.into_push_pull_output();
    {
        use embedded_hal::delay::DelayNs as _;
        tp_rst.set_high().unwrap();
        timer.delay_ms(10);
        tp_rst.set_low().unwrap();
        timer.delay_ms(10);
        tp_rst.set_high().unwrap();
        timer.delay_ms(100);
    }

    let tp_int = pins.gpio18.into_pull_up_input();
    tp_int.set_interrupt_enabled(GpioInterrupt::LevelLow, true);
    cortex_m::interrupt::free(|cs| {
        TP_INT_PIN.borrow(cs).replace(Some(tp_int));
    });

    let touch = cst328::CST328::new(i2c, &mut timer).unwrap();

    let mut alarm0 = timer.alarm_0().unwrap();
    alarm0.enable_interrupt();

    cortex_m::interrupt::free(|cs| {
        ALARM0.borrow(cs).replace(Some(alarm0));
        TIMER.borrow(cs).replace(Some(timer));
    });

    unsafe {
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::IO_IRQ_BANK0);
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::TIMER0_IRQ_0);
    }

    let dma = pac.DMA.split(&mut pac.RESETS);
    let pio = PioTransfer::Idle(
        dma.ch0,
        vec![Rgb565Pixel::default(); DISPLAY_SIZE.width as _].leak(),
        stolen_spi,
    );
    let buffer_provider = DrawBuffer {
        display,
        buffer: vec![Rgb565Pixel::default(); DISPLAY_SIZE.width as _].leak(),
        pio: Some(pio),
        stolen_pin: (dc_copy, cs_copy),
    };

    slint::platform::set_platform(Box::new(PicoBackend {
        window: Default::default(),
        buffer_provider: buffer_provider.into(),
        touch: touch.into(),
        backlight: Some(backlight).into(),
    }))
    .expect("backend already initialized");
}

struct PicoBackend<DrawBuffer, Touch, Backlight> {
    window: RefCell<Option<Rc<renderer::MinimalSoftwareWindow>>>,
    buffer_provider: RefCell<DrawBuffer>,
    touch: RefCell<Touch>,
    backlight: RefCell<Option<Backlight>>,
}

impl<
    DI: mipidsi::interface::Interface<Word = u8>,
    RST: OutputPin<Error = Infallible>,
    TO: WriteTarget<TransmittedWord = u8> + SpiBus + embedded_hal_nb::spi::FullDuplex,
    CH: SingleChannel,
    DC_: OutputPin<Error = Infallible>,
    CS_: OutputPin<Error = Infallible>,
    I2C: embedded_hal::i2c::I2c,
    BL: OutputPin<Error = Infallible>,
> slint::platform::Platform
    for PicoBackend<
        DrawBuffer<Display<DI, RST>, PioTransfer<TO, CH>, (DC_, CS_)>,
        cst328::CST328<I2C>,
        BL,
    >
{
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let window =
            renderer::MinimalSoftwareWindow::new(renderer::RepaintBufferType::ReusedBuffer);
        self.window.replace(Some(window.clone()));
        Ok(window)
    }

    fn duration_since_start(&self) -> core::time::Duration {
        let counter = cortex_m::interrupt::free(|cs| {
            TIMER.borrow(cs).borrow().as_ref().map(|t| t.get_counter().ticks()).unwrap_or_default()
        });
        core::time::Duration::from_micros(counter)
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let mut last_touch = None;

        self.window.borrow().as_ref().unwrap().set_size(DISPLAY_SIZE);

        loop {
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                window.draw_if_needed(|renderer| {
                    let mut buffer_provider = self.buffer_provider.borrow_mut();
                    renderer.render_by_line(&mut *buffer_provider);
                    buffer_provider.flush_frame();
                    if let Some(mut backlight) = self.backlight.take() {
                        backlight.set_high().unwrap();
                    }
                });

                // handle touch event
                let button = PointerEventButton::Left;
                if let Some(event) = self
                    .touch
                    .borrow_mut()
                    .read()
                    .map_err(|_| ())
                    .unwrap()
                    .map(|point| {
                        let position = slint::PhysicalPosition::new(
                            (point.x * DISPLAY_SIZE.width as f32) as _,
                            (point.y * DISPLAY_SIZE.height as f32) as _,
                        )
                        .to_logical(window.scale_factor());
                        match last_touch.replace(position) {
                            Some(_) => WindowEvent::PointerMoved { position },
                            None => WindowEvent::PointerPressed { position, button },
                        }
                    })
                    .or_else(|| {
                        last_touch
                            .take()
                            .map(|position| WindowEvent::PointerReleased { position, button })
                    })
                {
                    let is_pointer_release_event =
                        matches!(event, WindowEvent::PointerReleased { .. });

                    window.try_dispatch_event(event)?;

                    // removes hover state on widgets
                    if is_pointer_release_event {
                        window.try_dispatch_event(WindowEvent::PointerExited)?;
                    }
                    // Don't go to sleep after a touch event that forces a redraw
                    continue;
                }

                if window.has_active_animations() {
                    continue;
                }
            }

            let sleep_duration = match slint::platform::duration_until_next_timer_update() {
                None => None,
                Some(d) => {
                    let micros = d.as_micros() as u32;
                    if micros < 10 {
                        // Cannot wait for less than 10µs, or `schedule()` panics
                        continue;
                    } else {
                        Some(fugit::MicrosDurationU32::micros(micros))
                    }
                }
            };

            cortex_m::interrupt::free(|cs| {
                if let Some(duration) = sleep_duration {
                    ALARM0.borrow(cs).borrow_mut().as_mut().unwrap().schedule(duration).unwrap();
                }
                TP_INT_PIN
                    .borrow(cs)
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .set_interrupt_enabled(GpioInterrupt::LevelLow, true);
            });
            cortex_m::asm::wfe();
        }
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        use alloc::string::ToString;
        defmt::println!("{=str}", arguments.to_string());
    }
}

enum PioTransfer<TO: WriteTarget, CH: SingleChannel> {
    Idle(CH, &'static mut [TargetPixel], TO),
    Running(hal::dma::single_buffer::Transfer<CH, PartialReadBuffer, TO>),
}

impl<TO: WriteTarget<TransmittedWord = u8>, CH: SingleChannel> PioTransfer<TO, CH> {
    fn wait(self) -> (CH, &'static mut [TargetPixel], TO) {
        match self {
            PioTransfer::Idle(a, b, c) => (a, b, c),
            PioTransfer::Running(dma) => {
                let (a, b, to) = dma.wait();
                (a, b.0, to)
            }
        }
    }
}

struct DrawBuffer<Display, PioTransfer, Stolen> {
    display: Display,
    buffer: &'static mut [TargetPixel],
    pio: Option<PioTransfer>,
    stolen_pin: Stolen,
}

impl<
    DI: mipidsi::interface::Interface<Word = u8>,
    RST: OutputPin<Error = Infallible>,
    TO: WriteTarget<TransmittedWord = u8> + SpiBus,
    CH: SingleChannel,
    DC_: OutputPin<Error = Infallible>,
    CS_: OutputPin<Error = Infallible>,
> renderer::LineBufferProvider
    for &mut DrawBuffer<Display<DI, RST>, PioTransfer<TO, CH>, (DC_, CS_)>
{
    type TargetPixel = TargetPixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [TargetPixel]),
    ) {
        render_fn(&mut self.buffer[range.clone()]);

        // convert from little to big endian before sending to the DMA channel
        for x in &mut self.buffer[range.clone()] {
            *x = Rgb565Pixel(x.0.to_be())
        }
        let (ch, mut b, mut spi) = self.pio.take().unwrap().wait();
        // Flush SPI to ensure all DMA data bytes are clocked out before mipidsi
        // toggles DC for the next command. Without this, residual TX FIFO bytes
        // get clocked with DC=low and are misinterpreted as commands by the display.
        spi.flush().unwrap();
        core::mem::swap(&mut self.buffer, &mut b);

        // We send empty data just to get the device in the right window
        self.display
            .set_pixels(
                range.start as u16,
                line as _,
                range.end as u16,
                line as u16,
                core::iter::empty(),
            )
            .unwrap();

        self.stolen_pin.1.set_low().unwrap();
        self.stolen_pin.0.set_high().unwrap();
        let mut dma = hal::dma::single_buffer::Config::new(ch, PartialReadBuffer(b, range), spi);
        dma.pace(hal::dma::Pace::PreferSink);
        self.pio = Some(PioTransfer::Running(dma.start()));
    }
}

impl<
    DI: mipidsi::interface::Interface<Word = u8>,
    RST: OutputPin<Error = Infallible>,
    TO: WriteTarget<TransmittedWord = u8> + SpiBus + embedded_hal_nb::spi::FullDuplex,
    CH: SingleChannel,
    DC_: OutputPin<Error = Infallible>,
    CS_: OutputPin<Error = Infallible>,
> DrawBuffer<Display<DI, RST>, PioTransfer<TO, CH>, (DC_, CS_)>
{
    fn flush_frame(&mut self) {
        let (ch, b, mut spi) = self.pio.take().unwrap().wait();
        spi.flush().unwrap();
        self.stolen_pin.1.set_high().unwrap();

        // After the DMA operated, we need to empty the receive FIFO, otherwise
        // subsequent SPI operations may pick wrong values.
        // Continue to read as long as we don't get a Err(WouldBlock)
        while !embedded_hal_nb::spi::FullDuplex::read(&mut spi).is_err() {}

        self.pio = Some(PioTransfer::Idle(ch, b, spi));
    }
}

struct PartialReadBuffer(&'static mut [Rgb565Pixel], core::ops::Range<usize>);
unsafe impl embedded_dma::ReadBuffer for PartialReadBuffer {
    type Word = u8;

    unsafe fn read_buffer(&self) -> (*const <Self as embedded_dma::ReadBuffer>::Word, usize) {
        let act_slice = &self.0[self.1.clone()];
        (act_slice.as_ptr() as *const u8, act_slice.len() * core::mem::size_of::<Rgb565Pixel>())
    }
}

mod cst328 {
    use embedded_hal::i2c::I2c;
    use euclid::default::Point2D;

    const TP_ADDR: u8 = 0x1A;

    pub struct CST328<I2C: I2c> {
        i2c: I2C,
    }

    impl<I2C: I2c> CST328<I2C> {
        pub fn new(
            mut i2c: I2C,
            timer: &mut impl embedded_hal::delay::DelayNs,
        ) -> Result<Self, I2C::Error> {
            // Enter debug info mode
            i2c.write(TP_ADDR, &[0xD1, 0x01])?;
            timer.delay_ms(10);

            // Read chip ID (not used, but part of init sequence)
            i2c.write(TP_ADDR, &[0xD1, 0xFC])?;
            let mut chip_id = [0u8; 4];
            i2c.read(TP_ADDR, &mut chip_id)?;

            // Enter normal mode
            i2c.write(TP_ADDR, &[0xD1, 0x09])?;
            timer.delay_ms(10);

            Ok(Self { i2c })
        }

        pub fn read(&mut self) -> Result<Option<Point2D<f32>>, I2C::Error> {
            // Touch controller reports in native portrait orientation (240x320).
            // Display is rotated 90° to landscape, so swap axes and flip.
            const NATIVE_WIDTH: f32 = 240.0;
            const NATIVE_HEIGHT: f32 = 320.0;

            self.i2c.write(TP_ADDR, &[0xD0, 0x00])?;
            let mut data = [0u8; 27];
            self.i2c.read(TP_ADDR, &mut data)?;

            let finger_state = data[0] & 0x0F;
            if finger_state == 6 {
                let raw_x = ((data[1] as u16) << 4) | ((data[3] as u16) >> 4);
                let raw_y = ((data[2] as u16) << 4) | (data[3] as u16 & 0x0F);

                // For 90° rotation: landscape_x = raw_y, landscape_y = (native_width - raw_x)
                let x = raw_y as f32 / NATIVE_HEIGHT;
                let y = (NATIVE_WIDTH - raw_x as f32) / NATIVE_WIDTH;

                Ok(Some(euclid::point2(x, y)))
            } else {
                Ok(None)
            }
        }
    }
}

#[interrupt]
fn IO_IRQ_BANK0() {
    cortex_m::interrupt::free(|cs| {
        let mut pin = TP_INT_PIN.borrow(cs).borrow_mut();
        let pin = pin.as_mut().unwrap();
        pin.set_interrupt_enabled(GpioInterrupt::LevelLow, false);
        pin.clear_interrupt(GpioInterrupt::LevelLow);
    });
}

#[interrupt]
fn TIMER0_IRQ_0() {
    cortex_m::interrupt::free(|cs| {
        ALARM0.borrow(cs).borrow_mut().as_mut().unwrap().clear_interrupt();
    });
}

#[cfg(not(feature = "panic-probe"))]
#[inline(never)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Safety: it's ok to steal here since we are in the panic handler, and the rest of the code will not be run anymore
    let mut pac = unsafe { pac::Peripherals::steal() };

    let sio = hal::sio::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);

    // Re-init the display
    let mut watchdog = hal::watchdog::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let spi_sclk = pins.gpio10.into_function::<gpio::FunctionSpi>();
    let spi_mosi = pins.gpio11.into_function::<gpio::FunctionSpi>();

    let spi = hal::Spi::<_, _, _, 8>::new(pac.SPI1, (spi_mosi, spi_sclk));
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        SPI_ST7789VW_MAX_FREQ,
        &embedded_hal::spi::MODE_0,
    );

    let mut timer = Timer::new_timer0(pac.TIMER0, &mut pac.RESETS, &clocks);

    let rst = pins.gpio15.into_push_pull_output();
    let mut bl = pins.gpio16.into_push_pull_output();
    let dc = pins.gpio14.into_push_pull_output();
    let mut cs = pins.gpio13.into_push_pull_output();
    bl.set_high().unwrap();
    cs.set_high().unwrap();
    let peri_freq = clocks.peripheral_clock.freq();
    let spi = singleton!(:SpiRefCell = SpiRefCell::new((spi, SPI_ST7789VW_MAX_FREQ))).unwrap();
    let display_spi =
        SharedSpiWithFreq { refcell: spi, cs, freq: SPI_ST7789VW_MAX_FREQ, peri_freq };
    let mut buffer = [0_u8; 512];
    let di = mipidsi::interface::SpiInterface::new(display_spi, dc, &mut buffer);
    let mut display = mipidsi::Builder::new(mipidsi::models::ST7789, di)
        .reset_pin(rst)
        .display_size(DISPLAY_SIZE.height as _, DISPLAY_SIZE.width as _)
        .orientation(mipidsi::options::Orientation::new().rotate(mipidsi::options::Rotation::Deg90))
        .invert_colors(mipidsi::options::ColorInversion::Inverted)
        .init(&mut timer)
        .unwrap();

    use core::fmt::Write;
    use embedded_graphics::{
        mono_font::{MonoTextStyle, ascii::FONT_6X10},
        pixelcolor::Rgb565,
        prelude::*,
        text::Text,
    };

    display.fill_solid(&display.bounding_box(), Rgb565::new(0x00, 0x25, 0xff)).unwrap();

    struct WriteToScreen<'a, D> {
        x: i32,
        y: i32,
        width: i32,
        style: MonoTextStyle<'a, Rgb565>,
        display: &'a mut D,
    }
    let mut writer = WriteToScreen {
        x: 0,
        y: 1,
        width: display.bounding_box().size.width as i32 / 6 - 1,
        style: MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
        display: &mut display,
    };
    impl<'a, D: DrawTarget<Color = Rgb565>> Write for WriteToScreen<'a, D> {
        fn write_str(&mut self, mut s: &str) -> Result<(), core::fmt::Error> {
            while !s.is_empty() {
                let (x, y) = (self.x, self.y);
                let end_of_line = s
                    .find(|c| {
                        if c == '\n' || self.x > self.width {
                            self.x = 0;
                            self.y += 1;
                            true
                        } else {
                            self.x += 1;
                            false
                        }
                    })
                    .unwrap_or(s.len());
                let (line, rest) = s.split_at(end_of_line);
                let sz = self.style.font.character_size;
                Text::new(line, Point::new(x * sz.width as i32, y * sz.height as i32), self.style)
                    .draw(self.display)
                    .map_err(|_| core::fmt::Error)?;
                s = rest.strip_prefix('\n').unwrap_or(rest);
            }
            Ok(())
        }
    }
    write!(writer, "{}", info).unwrap();

    loop {
        use embedded_hal::delay::DelayNs as _;
        timer.delay_ms(100);
    }
}
