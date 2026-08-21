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
use defmt_rtt as _;
use embedded_alloc::LlffHeap as Heap;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::{ErrorType, Operation, SpiBus, SpiDevice};
use fugit::{Hertz, RateExtU32};
use hal::dma::{DMAExt, SingleChannel, WriteTarget};
use hal::gpio::{self, Interrupt as GpioInterrupt};
use hal::timer::{Alarm, Alarm0};
use pac::interrupt;
#[cfg(feature = "panic-probe")]
use panic_probe as _;
use renderer::Rgb565Pixel;

#[path = "../pico2_st7789/rp_pico2.rs"]
mod rp_pico2;
use rp_pico2::hal::{self, Timer, pac, prelude::*, timer::CopyableTimer0};
use slint::platform::{PointerEventButton, WindowEvent, software_renderer as renderer};

const HEAP_SIZE: usize = 400 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

type IrqPin = gpio::Pin<gpio::bank0::Gpio17, gpio::FunctionSio<gpio::SioInput>, gpio::PullUp>;
static IRQ_PIN: Mutex<RefCell<Option<IrqPin>>> = Mutex::new(RefCell::new(None));

static ALARM0: Mutex<RefCell<Option<Alarm0<CopyableTimer0>>>> = Mutex::new(RefCell::new(None));
static TIMER: Mutex<RefCell<Option<Timer<CopyableTimer0>>>> = Mutex::new(RefCell::new(None));

// The peripheral clock frequency resulting from the default clock setup
const PERI_CLOCK_FREQ: Hertz<u32> = Hertz::<u32>::Hz(150_000_000);

// The Pico-ResTouch-LCD-3.5 drives the ILI9488 through a serial-to-parallel
// converter. Waveshare states writes were tested up to 60 MHz; a quarter of
// the peripheral clock is the closest SPI divider below that.
const SPI_LCD_MAX_FREQ: Hertz<u32> = Hertz::<u32>::Hz(PERI_CLOCK_FREQ.raw() / 4);

const DISPLAY_SIZE: slint::PhysicalSize = slint::PhysicalSize::new(480, 320);

/// The Pixel type of the backing store
pub type TargetPixel = Rgb565Pixel;

type SpiPins = (
    gpio::Pin<gpio::bank0::Gpio11, gpio::FunctionSpi, gpio::PullDown>,
    gpio::Pin<gpio::bank0::Gpio12, gpio::FunctionSpi, gpio::PullDown>,
    gpio::Pin<gpio::bank0::Gpio10, gpio::FunctionSpi, gpio::PullDown>,
);

type EnabledSpi = hal::Spi<hal::spi::Enabled, pac::SPI1, SpiPins, 8>;
type SpiRefCell = RefCell<(EnabledSpi, Hertz<u32>)>;

#[derive(Clone)]
struct SharedSpiWithFreq<CS> {
    refcell: &'static SpiRefCell,
    cs: CS,
    freq: Hertz<u32>,
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
            // the touchscreen and the LCD have different frequencies
            borrowed.0.set_baudrate(PERI_CLOCK_FREQ, self.freq);
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

/// Driver for the ILI9488 behind the serial-to-parallel converter of the
/// Pico-ResTouch-LCD-3.5.
///
/// The converter latches whole 16-bit words onto the controller's parallel
/// bus, so every command byte must be sent in its own chip-select cycle and
/// every parameter or pixel is a 16-bit big-endian word (which also makes
/// RGB565 usable, unlike the ILI9488's plain SPI mode).
struct Ili9488<SPI, DC> {
    spi: SPI,
    dc: DC,
}

impl<SPI: SpiDevice, DC: OutputPin<Error = Infallible>> Ili9488<SPI, DC> {
    fn new(spi: SPI, dc: DC, delay: &mut impl DelayNs) -> Result<Self, SPI::Error> {
        let mut this = Self { spi, dc };
        this.command(0x21, &[])?; // display inversion on (panel is inverted)
        this.command(0xC2, &[0x33])?; // power control 3
        this.command(0xC5, &[0x00, 0x1E, 0x80])?; // VCOM control
        this.command(0xB1, &[0xB0])?; // frame rate 70Hz
        this.command(
            0xE0, // positive gamma
            &[
                0x00, 0x13, 0x18, 0x04, 0x0F, 0x06, 0x3A, 0x56, 0x4D, 0x03, 0x0A, 0x06, 0x30, 0x3E,
                0x0F,
            ],
        )?;
        this.command(
            0xE1, // negative gamma
            &[
                0x00, 0x13, 0x18, 0x01, 0x11, 0x06, 0x38, 0x34, 0x4D, 0x06, 0x0D, 0x0B, 0x31, 0x37,
                0x0F,
            ],
        )?;
        this.command(0x3A, &[0x55])?; // 16 bit/pixel
        // Landscape orientation: X-Y exchange in MADCTL; gate and source scan
        // direction in the display function control put the origin on the
        // side away from the display's cable connector
        this.command(0xB6, &[0x00, 0x62])?;
        this.command(0x36, &[0x28])?;
        this.command(0x11, &[])?; // sleep out
        delay.delay_ms(120);
        // Clear the frame memory before turning the display on
        use embedded_graphics::draw_target::DrawTarget;
        use embedded_graphics::prelude::*;
        this.fill_solid(&this.bounding_box(), embedded_graphics::pixelcolor::Rgb565::BLACK)?;
        this.command(0x29, &[])?; // display on
        Ok(this)
    }

    fn command(&mut self, cmd: u8, params: &[u8]) -> Result<(), SPI::Error> {
        self.dc.set_low().unwrap();
        self.spi.write(&[cmd])?;
        self.dc.set_high().unwrap();
        if !params.is_empty() {
            // One 16-bit big-endian word per parameter, batched in a single
            // transaction. No command takes more than 15 parameters.
            let mut words = [0u8; 30];
            for (word, &p) in words.chunks_exact_mut(2).zip(params) {
                word[1] = p;
            }
            self.spi.write(&words[..params.len() * 2])?;
        }
        Ok(())
    }

    /// Sets the drawing window (inclusive coordinates) and issues the memory
    /// write command. Pixel data can then be streamed with DC high.
    fn set_window(&mut self, xs: u16, ys: u16, xe: u16, ye: u16) -> Result<(), SPI::Error> {
        self.command(0x2A, &[(xs >> 8) as u8, xs as u8, (xe >> 8) as u8, xe as u8])?;
        self.command(0x2B, &[(ys >> 8) as u8, ys as u8, (ye >> 8) as u8, ye as u8])?;
        self.command(0x2C, &[])
    }
}

impl<SPI: SpiDevice, DC: OutputPin<Error = Infallible>>
    embedded_graphics::geometry::OriginDimensions for Ili9488<SPI, DC>
{
    fn size(&self) -> embedded_graphics::geometry::Size {
        embedded_graphics::geometry::Size::new(DISPLAY_SIZE.width, DISPLAY_SIZE.height)
    }
}

impl<SPI: SpiDevice, DC: OutputPin<Error = Infallible>> embedded_graphics::draw_target::DrawTarget
    for Ili9488<SPI, DC>
{
    type Color = embedded_graphics::pixelcolor::Rgb565;
    type Error = SPI::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        use embedded_graphics::prelude::*;
        for Pixel(coord, color) in pixels {
            if coord.x >= 0
                && coord.y >= 0
                && (coord.x as u32) < DISPLAY_SIZE.width
                && (coord.y as u32) < DISPLAY_SIZE.height
            {
                self.set_window(coord.x as u16, coord.y as u16, coord.x as u16, coord.y as u16)?;
                self.spi.write(
                    &embedded_graphics::pixelcolor::raw::RawU16::from(color)
                        .into_inner()
                        .to_be_bytes(),
                )?;
            }
        }
        Ok(())
    }

    fn fill_solid(
        &mut self,
        area: &embedded_graphics::primitives::Rectangle,
        color: Self::Color,
    ) -> Result<(), Self::Error> {
        use embedded_graphics::prelude::*;
        let area = area.intersection(&self.bounding_box());
        let Some(bottom_right) = area.bottom_right() else { return Ok(()) };
        self.set_window(
            area.top_left.x as u16,
            area.top_left.y as u16,
            bottom_right.x as u16,
            bottom_right.y as u16,
        )?;
        let raw = embedded_graphics::pixelcolor::raw::RawU16::from(color).into_inner();
        let mut chunk = [0u8; 64];
        for pair in chunk.chunks_exact_mut(2) {
            pair.copy_from_slice(&raw.to_be_bytes());
        }
        let mut remaining = area.size.width as usize * area.size.height as usize * 2;
        while remaining > 0 {
            let n = remaining.min(chunk.len());
            self.spi.write(&chunk[..n])?;
            remaining -= n;
        }
        Ok(())
    }
}

/// The serial-to-parallel converter's shift counter powers up (or is left by
/// an interrupted transfer) in an arbitrary state, which can make it drop or
/// corrupt the first command — showing up as e.g. randomly missing display
/// inversion. Clock out one dummy word so the chip-select edge leaves the
/// counter synchronized, then reset the display controller to undo whatever
/// the dummy word may have latched.
fn sync_converter_and_reset_display(
    spi: &mut EnabledSpi,
    cs: &mut impl OutputPin<Error = Infallible>,
    rst: &mut impl OutputPin<Error = Infallible>,
    delay: &mut impl DelayNs,
) {
    cs.set_low().unwrap();
    SpiBus::write(spi, &[0u8; 2]).ok();
    SpiBus::flush(spi).ok();
    cs.set_high().unwrap();

    rst.set_high().unwrap();
    delay.delay_ms(10);
    rst.set_low().unwrap();
    delay.delay_ms(10);
    rst.set_high().unwrap();
    delay.delay_ms(120);
}

pub fn init() {
    let mut pac = pac::Peripherals::take().unwrap();

    let mut watchdog = hal::watchdog::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        rp_pico2::XOSC_CRYSTAL_FREQ,
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
    let pins = rp_pico2::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);

    let mut touch_cs = pins.gpio16.into_push_pull_output();
    touch_cs.set_high().unwrap();
    let touch_irq = pins.gpio17.into_pull_up_input();
    touch_irq.set_interrupt_enabled(GpioInterrupt::LevelLow, true);
    cortex_m::interrupt::free(|cs| {
        IRQ_PIN.borrow(cs).replace(Some(touch_irq));
    });

    let mut rst = pins.gpio15.into_push_pull_output();
    let backlight = pins.gpio13.into_push_pull_output();

    let dc = pins.gpio8.into_push_pull_output();
    let mut cs = pins.gpio9.into_push_pull_output();

    let spi_sclk = pins.gpio10.into_function::<gpio::FunctionSpi>();
    let spi_mosi = pins.gpio11.into_function::<gpio::FunctionSpi>();
    let spi_miso = pins.gpio12.into_function::<gpio::FunctionSpi>();

    let spi = hal::Spi::new(pac.SPI1, (spi_mosi, spi_miso, spi_sclk));
    let mut spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        SPI_LCD_MAX_FREQ,
        &embedded_hal::spi::MODE_0,
    );

    sync_converter_and_reset_display(&mut spi, &mut cs, &mut rst, &mut timer);

    // SAFETY: This is not safe :-(  But we need to access the SPI and its control pins for the DMA
    let (dc_copy, cs_copy) =
        unsafe { (core::ptr::read(&dc as *const _), core::ptr::read(&cs as *const _)) };
    let stolen_spi = unsafe { core::ptr::read(&spi as *const _) };

    let spi = singleton!(:SpiRefCell = SpiRefCell::new((spi, 0.Hz()))).unwrap();

    let display_spi = SharedSpiWithFreq { refcell: spi, cs, freq: SPI_LCD_MAX_FREQ };
    let display = Ili9488::new(display_spi, dc, &mut timer).unwrap();

    let touch = xpt2046::XPT2046::new(
        &IRQ_PIN,
        SharedSpiWithFreq { refcell: spi, cs: touch_cs, freq: xpt2046::SPI_FREQ },
    )
    .unwrap();

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
    let dma_transfer = DmaTransfer::Idle(
        dma.ch0,
        vec![Rgb565Pixel::default(); DISPLAY_SIZE.width as _].leak(),
        stolen_spi,
    );
    let buffer_provider = DrawBuffer {
        display,
        buffer: vec![Rgb565Pixel::default(); DISPLAY_SIZE.width as _].leak(),
        dma: Some(dma_transfer),
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
    DSPI: SpiDevice,
    DC: OutputPin<Error = Infallible>,
    TO: WriteTarget<TransmittedWord = u8> + embedded_hal_nb::spi::FullDuplex + SpiBus,
    CH: SingleChannel,
    DC_: OutputPin<Error = Infallible>,
    CS_: OutputPin<Error = Infallible>,
    IRQ: InputPin<Error = Infallible>,
    SPI: SpiDevice,
    BL: OutputPin<Error = Infallible>,
> slint::platform::Platform
    for PicoBackend<
        DrawBuffer<Ili9488<DSPI, DC>, DmaTransfer<TO, CH>, (DC_, CS_)>,
        xpt2046::XPT2046<IRQ, SPI>,
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

                    window.dispatch_event_with_result(event)?;

                    // removes hover state on widgets
                    if is_pointer_release_event {
                        window.dispatch_event_with_result(WindowEvent::PointerExited)?;
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
                    let micros = u32::try_from(d.as_micros()).unwrap_or(u32::MAX);
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

                IRQ_PIN
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

enum DmaTransfer<TO: WriteTarget, CH: SingleChannel> {
    Idle(CH, &'static mut [TargetPixel], TO),
    Running(hal::dma::single_buffer::Transfer<CH, PartialReadBuffer, TO>),
}

impl<TO: WriteTarget<TransmittedWord = u8>, CH: SingleChannel> DmaTransfer<TO, CH> {
    fn wait(self) -> (CH, &'static mut [TargetPixel], TO) {
        match self {
            DmaTransfer::Idle(a, b, c) => (a, b, c),
            DmaTransfer::Running(dma) => {
                let (a, b, to) = dma.wait();
                (a, b.0, to)
            }
        }
    }
}

struct DrawBuffer<Display, DmaTransfer, Stolen> {
    display: Display,
    buffer: &'static mut [TargetPixel],
    dma: Option<DmaTransfer>,
    stolen_pin: Stolen,
}

impl<
    DSPI: SpiDevice,
    DC: OutputPin<Error = Infallible>,
    TO: WriteTarget<TransmittedWord = u8> + SpiBus,
    CH: SingleChannel,
    DC_: OutputPin<Error = Infallible>,
    CS_: OutputPin<Error = Infallible>,
> renderer::LineBufferProvider
    for &mut DrawBuffer<Ili9488<DSPI, DC>, DmaTransfer<TO, CH>, (DC_, CS_)>
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
        let (ch, mut b, mut spi) = self.dma.take().unwrap().wait();
        // The DMA is done once the last byte was accepted by the SPI FIFO, so
        // wait until the FIFO is drained before DC changes for the next
        // command, otherwise the tail of the line would be taken as commands
        spi.flush().ok();
        core::mem::swap(&mut self.buffer, &mut b);

        // Set the window and issue the memory write command, then stream the
        // pixels through the DMA channel
        self.display
            .set_window(range.start as u16, line as u16, range.end as u16 - 1, line as u16)
            .unwrap();

        self.stolen_pin.1.set_low().unwrap();
        self.stolen_pin.0.set_high().unwrap();
        let mut dma = hal::dma::single_buffer::Config::new(ch, PartialReadBuffer(b, range), spi);
        dma.pace(hal::dma::Pace::PreferSink);
        self.dma = Some(DmaTransfer::Running(dma.start()));
    }
}

impl<
    DSPI: SpiDevice,
    DC: OutputPin<Error = Infallible>,
    TO: WriteTarget<TransmittedWord = u8> + embedded_hal_nb::spi::FullDuplex,
    CH: SingleChannel,
    DC_: OutputPin<Error = Infallible>,
    CS_: OutputPin<Error = Infallible>,
> DrawBuffer<Ili9488<DSPI, DC>, DmaTransfer<TO, CH>, (DC_, CS_)>
{
    fn flush_frame(&mut self) {
        let (ch, b, mut spi) = self.dma.take().unwrap().wait();
        self.stolen_pin.1.set_high().unwrap();

        // After the DMA operated, we need to empty the receive FIFO, otherwise the touch screen
        // driver will pick wrong values.
        // Continue to read as long as we don't get a Err(WouldBlock)
        while !spi.read().is_err() {}

        self.dma = Some(DmaTransfer::Idle(ch, b, spi));
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

mod xpt2046 {
    use core::cell::RefCell;
    use cortex_m::interrupt::Mutex;
    use embedded_hal::digital::InputPin;
    use embedded_hal::spi::SpiDevice;
    use euclid::default::Point2D;
    use fugit::Hertz;

    pub const SPI_FREQ: Hertz<u32> = Hertz::<u32>::Hz(3_000_000);

    pub struct XPT2046<IRQ: InputPin + 'static, SPI: SpiDevice> {
        irq: &'static Mutex<RefCell<Option<IRQ>>>,
        spi: SPI,
        pressed: bool,
        last_point: Option<Point2D<f32>>,
    }

    impl<PinE, IRQ: InputPin<Error = PinE>, SPI: SpiDevice> XPT2046<IRQ, SPI> {
        pub fn new(irq: &'static Mutex<RefCell<Option<IRQ>>>, spi: SPI) -> Result<Self, PinE> {
            Ok(Self { irq, spi, pressed: false, last_point: None })
        }

        pub fn read(&mut self) -> Result<Option<Point2D<f32>>, Error<PinE, SPI::Error>> {
            const PRESS_THRESHOLD: i32 = -25_000;
            const RELEASE_THRESHOLD: i32 = -30_000;
            let threshold = if self.pressed { RELEASE_THRESHOLD } else { PRESS_THRESHOLD };
            let was_pressed = self.pressed;
            self.pressed = false;

            if cortex_m::interrupt::free(|cs| {
                self.irq.borrow(cs).borrow_mut().as_mut().unwrap().is_low()
            })
            .map_err(|e| Error::Pin(e))?
            {
                // On this panel the Y+ channel measures along the 480 pixel
                // axis and the X+ channel along the 320 pixel axis (mirrored)
                const CMD_X_READ: u8 = 0b11010000;
                const CMD_Y_READ: u8 = 0b10010000;
                const CMD_Z1_READ: u8 = 0b10110000;
                const CMD_Z2_READ: u8 = 0b11000000;

                // These numbers are derived from the calibration factors in
                // Waveshare's demo code.
                const MIN_X: u32 = 2400;
                const MAX_X: u32 = 31200;
                const MIN_Y: u32 = 3190;
                const MAX_Y: u32 = 30390;

                macro_rules! xchg {
                    ($byte:expr) => {{
                        let mut b = [0, $byte, 0, 0];
                        self.spi.transfer_in_place(&mut b).map_err(|e| Error::Transfer(e))?;
                        let [_, _, h, l] = b;
                        ((h as u32) << 8) | (l as u32)
                    }};
                }

                // Read one channel several times, drop the extremes and
                // average the rest, like Waveshare's demo does. The first
                // conversion after switching channels is discarded, and the
                // conversions are spread out in time: the touch panel sits
                // right on top of the LCD, so it needs a while to settle
                // after pixels were written
                macro_rules! sample_channel {
                    ($cmd:expr) => {{
                        let _ = xchg!($cmd);
                        let mut samples = [0u32; 5];
                        for s in samples.iter_mut() {
                            // ~200µs, like the delay in Waveshare's demo
                            cortex_m::asm::delay(30_000);
                            *s = xchg!($cmd);
                        }
                        samples.sort_unstable();
                        (samples[1] + samples[2] + samples[3]) / 3
                    }};
                }

                let z1 = xchg!(CMD_Z1_READ);
                let z2 = xchg!(CMD_Z2_READ);
                let z = z1 as i32 - z2 as i32;

                if z < threshold {
                    return Ok(None);
                }

                // Take two independent measurements per axis and reject the
                // reading unless they agree, like Waveshare's demo does:
                // readings on this panel occasionally jump
                let x1 = sample_channel!(CMD_Y_READ);
                let y1 = sample_channel!(CMD_X_READ);
                let x2 = sample_channel!(CMD_Y_READ);
                let y2 = sample_channel!(CMD_X_READ);

                // 50 counts in the demo's 12 bit scale
                const ERR_RANGE: u32 = 50 << 3;
                if x1.abs_diff(x2) > ERR_RANGE || y1.abs_diff(y2) > ERR_RANGE {
                    // Inconsistent reading: keep the previous position while
                    // pressed rather than reporting a bogus one
                    self.pressed = was_pressed;
                    return Ok(if was_pressed { self.last_point } else { None });
                }

                let point = Point2D::new((x1 + x2) / 2, i16::MAX as u32 - (y1 + y2) / 2);

                let z1 = xchg!(CMD_Z1_READ);
                let z2 = xchg!(CMD_Z2_READ);
                let z = z1 as i32 - z2 as i32;

                if z < RELEASE_THRESHOLD {
                    return Ok(None);
                }

                self.pressed = true;
                let point = euclid::point2(
                    (point.x.saturating_sub(MIN_X) as f32 / (MAX_X - MIN_X) as f32).min(1.0),
                    (point.y.saturating_sub(MIN_Y) as f32 / (MAX_Y - MIN_Y) as f32).min(1.0),
                );
                self.last_point = Some(point);
                Ok(Some(point))
            } else {
                Ok(None)
            }
        }
    }

    pub enum Error<PinE, TransferE> {
        Pin(PinE),
        Transfer(TransferE),
    }
}

#[interrupt]
fn IO_IRQ_BANK0() {
    cortex_m::interrupt::free(|cs| {
        let mut pin = IRQ_PIN.borrow(cs).borrow_mut();
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
    let pins = rp_pico2::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);
    let mut led = pins.led.into_push_pull_output();
    led.set_high().unwrap();

    // Re-init the display
    let mut watchdog = hal::watchdog::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        rp_pico2::XOSC_CRYSTAL_FREQ,
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
    let spi_miso = pins.gpio12.into_function::<gpio::FunctionSpi>();

    let spi = hal::Spi::<_, _, _, 8>::new(pac.SPI1, (spi_mosi, spi_miso, spi_sclk));
    let mut spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        4_000_000u32.Hz(),
        &embedded_hal::spi::MODE_0,
    );

    let mut timer = Timer::new_timer0(pac.TIMER0, &mut pac.RESETS, &clocks);

    let mut rst = pins.gpio15.into_push_pull_output();
    let mut bl = pins.gpio13.into_push_pull_output();
    let dc = pins.gpio8.into_push_pull_output();
    let mut cs = pins.gpio9.into_push_pull_output();
    bl.set_high().unwrap();

    sync_converter_and_reset_display(&mut spi, &mut cs, &mut rst, &mut timer);

    let spi = singleton!(:SpiRefCell = SpiRefCell::new((spi, 0.Hz()))).unwrap();
    let display_spi = SharedSpiWithFreq { refcell: spi, cs, freq: SPI_LCD_MAX_FREQ };
    let mut display = Ili9488::new(display_spi, dc, &mut timer).unwrap();

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
        led.set_low().unwrap();
        timer.delay_ms(100);
        led.set_high().unwrap();
    }
}
