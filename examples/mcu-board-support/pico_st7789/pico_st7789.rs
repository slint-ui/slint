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
use rp_pico::hal::{self, pac, prelude::*, Timer};
use slint::platform::{software_renderer as renderer, PointerEventButton, WindowEvent};

const HEAP_SIZE: usize = 200 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

type IrqPin = gpio::Pin<gpio::bank0::Gpio17, gpio::FunctionSio<gpio::SioInput>, gpio::PullUp>;
static IRQ_PIN: Mutex<RefCell<Option<IrqPin>>> = Mutex::new(RefCell::new(None));

static ALARM0: Mutex<RefCell<Option<Alarm0>>> = Mutex::new(RefCell::new(None));
static TIMER: Mutex<RefCell<Option<Timer>>> = Mutex::new(RefCell::new(None));

// 16ns for serial clock cycle (write), page 43 of https://www.waveshare.com/w/upload/a/ae/ST7789_Datasheet.pdf
const SPI_ST7789VW_MAX_FREQ: Hertz<u32> = Hertz::<u32>::Hz(62_500_000);

const DISPLAY_SIZE: slint::PhysicalSize = slint::PhysicalSize::new(320, 240);

/// The Pixel type of the backing store
pub type TargetPixel = Rgb565Pixel;

type SpiPins = (
    gpio::Pin<gpio::bank0::Gpio11, gpio::FunctionSpi, gpio::PullDown>,
    gpio::Pin<gpio::bank0::Gpio12, gpio::FunctionSpi, gpio::PullDown>,
    gpio::Pin<gpio::bank0::Gpio10, gpio::FunctionSpi, gpio::PullDown>,
);

type EnabledSpi = hal::Spi<hal::spi::Enabled, pac::SPI1, SpiPins, 8>;
type SpiRefCell = RefCell<(EnabledSpi, Hertz<u32>)>;
type Display<DI, RST> = mipidsi::Display<DI, mipidsi::models::ST7789, RST>;

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
            borrowed.0.set_baudrate(125_000_000u32.Hz(), self.freq);
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

    unsafe { ALLOCATOR.init(core::ptr::addr_of_mut!(HEAP) as usize, HEAP_SIZE) }

    let mut timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    let sio = hal::sio::Sio::new(pac.SIO);
    let pins = rp_pico::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);

    let mut touch_cs = pins.gpio16.into_push_pull_output();
    touch_cs.set_high().unwrap();
    let touch_irq = pins.gpio17.into_pull_up_input();
    touch_irq.set_interrupt_enabled(GpioInterrupt::LevelLow, true);
    cortex_m::interrupt::free(|cs| {
        IRQ_PIN.borrow(cs).replace(Some(touch_irq));
    });

    let rst = pins.gpio15.into_push_pull_output();
    let backlight = pins.gpio13.into_push_pull_output();

    let dc = pins.gpio8.into_push_pull_output();
    let cs = pins.gpio9.into_push_pull_output();

    let spi_sclk = pins.gpio10.into_function::<gpio::FunctionSpi>();
    let spi_mosi = pins.gpio11.into_function::<gpio::FunctionSpi>();
    let spi_miso = pins.gpio12.into_function::<gpio::FunctionSpi>();

    let spi = hal::Spi::new(pac.SPI1, (spi_mosi, spi_miso, spi_sclk));
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        SPI_ST7789VW_MAX_FREQ,
        &embedded_hal::spi::MODE_3,
    );

    // SAFETY: This is not safe :-(  But we need to access the SPI and its control pins for the PIO
    let (dc_copy, cs_copy) =
        unsafe { (core::ptr::read(&dc as *const _), core::ptr::read(&cs as *const _)) };
    let stolen_spi = unsafe { core::ptr::read(&spi as *const _) };

    let spi = singleton!(:SpiRefCell = SpiRefCell::new((spi, 0.Hz()))).unwrap();
    let mipidsi_buffer = singleton!(:[u8; 512] = [0; 512]).unwrap();

    let display_spi = SharedSpiWithFreq { refcell: spi, cs, freq: SPI_ST7789VW_MAX_FREQ };
    let di = mipidsi::interface::SpiInterface::new(display_spi, dc, mipidsi_buffer);
    let display = mipidsi::Builder::new(mipidsi::models::ST7789, di)
        .reset_pin(rst)
        .display_size(DISPLAY_SIZE.height as _, DISPLAY_SIZE.width as _)
        .orientation(mipidsi::options::Orientation::new().rotate(mipidsi::options::Rotation::Deg90))
        .invert_colors(mipidsi::options::ColorInversion::Inverted)
        .init(&mut timer)
        .unwrap();

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
        pac::NVIC::unmask(pac::Interrupt::IO_IRQ_BANK0);
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
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
        TO: WriteTarget<TransmittedWord = u8> + embedded_hal_nb::spi::FullDuplex,
        CH: SingleChannel,
        DC_: OutputPin<Error = Infallible>,
        CS_: OutputPin<Error = Infallible>,
        IRQ: InputPin<Error = Infallible>,
        SPI: SpiDevice,
        BL: OutputPin<Error = Infallible>,
    > slint::platform::Platform
    for PicoBackend<
        DrawBuffer<Display<DI, RST>, PioTransfer<TO, CH>, (DC_, CS_)>,
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
        TO: WriteTarget<TransmittedWord = u8>,
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

        /* -- Send the pixel without DMA
        self.display.set_pixels(
            range.start as _,
            line as _,
            range.end as _,
            line as _,
            self.buffer[range.clone()]
                .iter()
                .map(|x| embedded_graphics::pixelcolor::raw::RawU16::new(x.0).into()),
        );
        return;*/

        // convert from little to big endian before sending to the DMA channel
        for x in &mut self.buffer[range.clone()] {
            *x = Rgb565Pixel(x.0.to_be())
        }
        let (ch, mut b, spi) = self.pio.take().unwrap().wait();
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
        /*let (a, b, c) = dma.start().wait();
        self.pio = Some(PioTransfer::Idle(a, b.0, c));*/
    }
}

impl<
        DI: mipidsi::interface::Interface<Word = u8>,
        RST: OutputPin<Error = Infallible>,
        TO: WriteTarget<TransmittedWord = u8> + embedded_hal_nb::spi::FullDuplex,
        CH: SingleChannel,
        DC_: OutputPin<Error = Infallible>,
        CS_: OutputPin<Error = Infallible>,
    > DrawBuffer<Display<DI, RST>, PioTransfer<TO, CH>, (DC_, CS_)>
{
    fn flush_frame(&mut self) {
        let (ch, b, mut spi) = self.pio.take().unwrap().wait();
        self.stolen_pin.1.set_high().unwrap();

        // After the DMA operated, we need to empty the receive FIFO, otherwise the touch screen
        // driver will pick wrong values.
        // Continue to read as long as we don't get a Err(WouldBlock)
        while !spi.read().is_err() {}

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
    }

    impl<PinE, IRQ: InputPin<Error = PinE>, SPI: SpiDevice> XPT2046<IRQ, SPI> {
        pub fn new(irq: &'static Mutex<RefCell<Option<IRQ>>>, spi: SPI) -> Result<Self, PinE> {
            Ok(Self { irq, spi, pressed: false })
        }

        pub fn read(&mut self) -> Result<Option<Point2D<f32>>, Error<PinE, SPI::Error>> {
            const PRESS_THRESHOLD: i32 = -25_000;
            const RELEASE_THRESHOLD: i32 = -30_000;
            let threshold = if self.pressed { RELEASE_THRESHOLD } else { PRESS_THRESHOLD };
            self.pressed = false;

            if cortex_m::interrupt::free(|cs| {
                self.irq.borrow(cs).borrow_mut().as_mut().unwrap().is_low()
            })
            .map_err(|e| Error::Pin(e))?
            {
                const CMD_X_READ: u8 = 0b10010000;
                const CMD_Y_READ: u8 = 0b11010000;
                const CMD_Z1_READ: u8 = 0b10110000;
                const CMD_Z2_READ: u8 = 0b11000000;

                // These numbers were measured approximately.
                const MIN_X: u32 = 1900;
                const MAX_X: u32 = 30300;
                const MIN_Y: u32 = 2300;
                const MAX_Y: u32 = 30300;

                macro_rules! xchg {
                    ($byte:expr) => {{
                        let mut b = [0, $byte, 0, 0];
                        self.spi.transfer_in_place(&mut b).map_err(|e| Error::Transfer(e))?;
                        let [_, _, h, l] = b;
                        ((h as u32) << 8) | (l as u32)
                    }};
                }

                let z1 = xchg!(CMD_Z1_READ);
                let z2 = xchg!(CMD_Z2_READ);
                let z = z1 as i32 - z2 as i32;

                if z < threshold {
                    return Ok(None);
                }

                let mut point = Point2D::new(0u32, 0u32);
                for _ in 0..10 {
                    let y = xchg!(CMD_Y_READ);
                    let x = xchg!(CMD_X_READ);
                    point += euclid::vec2(i16::MAX as u32 - x, y)
                }
                point /= 10;

                let z1 = xchg!(CMD_Z1_READ);
                let z2 = xchg!(CMD_Z2_READ);
                let z = z1 as i32 - z2 as i32;

                if z < RELEASE_THRESHOLD {
                    return Ok(None);
                }

                self.pressed = true;
                Ok(Some(euclid::point2(
                    point.x.saturating_sub(MIN_X) as f32 / (MAX_X - MIN_X) as f32,
                    point.y.saturating_sub(MIN_Y) as f32 / (MAX_Y - MIN_Y) as f32,
                )))
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
fn TIMER_IRQ_0() {
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
    let pins = rp_pico::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);
    let mut led = pins.led.into_push_pull_output();
    led.set_high().unwrap();

    // Re-init the display
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

    let spi_sclk = pins.gpio10.into_function::<gpio::FunctionSpi>();
    let spi_mosi = pins.gpio11.into_function::<gpio::FunctionSpi>();
    let spi_miso = pins.gpio12.into_function::<gpio::FunctionSpi>();

    let spi = hal::Spi::<_, _, _, 8>::new(pac.SPI1, (spi_mosi, spi_miso, spi_sclk));
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        4_000_000u32.Hz(),
        &embedded_hal::spi::MODE_3,
    );

    let mut timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    let rst = pins.gpio15.into_push_pull_output();
    let mut bl = pins.gpio13.into_push_pull_output();
    let dc = pins.gpio8.into_push_pull_output();
    let cs = pins.gpio9.into_push_pull_output();
    bl.set_high().unwrap();
    let spi = singleton!(:SpiRefCell = SpiRefCell::new((spi, 0.Hz()))).unwrap();
    let display_spi = SharedSpiWithFreq { refcell: spi, cs, freq: SPI_ST7789VW_MAX_FREQ };
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
        mono_font::{ascii::FONT_6X10, MonoTextStyle},
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
