// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

extern crate alloc;

use alloc::vec;
use core::cell::RefCell;
use core::convert::Infallible;
use cortex_m::interrupt::Mutex;
use cortex_m::singleton;
pub use cortex_m_rt::entry;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_hal::blocking::spi::Transfer;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use embedded_hal::spi::FullDuplex;
use embedded_time::rate::*;
use hal::dma::{DMAExt, SingleChannel, WriteTarget};
use rp_pico::hal::gpio::{self, Interrupt as GpioInterrupt};
use rp_pico::hal::pac::interrupt;
use rp_pico::hal::timer::{Alarm, Alarm0};
use rp_pico::hal::{self, pac, prelude::*, Timer};

use defmt_rtt as _; // global logger

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[alloc_error_handler]
fn oom(layout: core::alloc::Layout) -> ! {
    panic!("Out of memory {:?}", layout);
}
use alloc_cortex_m::CortexMHeap;

use crate::{Devices, PhysicalLength, PhysicalSize};

mod display_interface_spi;

const HEAP_SIZE: usize = 200 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

type IrqPin = gpio::Pin<gpio::bank0::Gpio17, gpio::PullUpInput>;
static IRQ_PIN: Mutex<RefCell<Option<IrqPin>>> = Mutex::new(RefCell::new(None));

static ALARM0: Mutex<RefCell<Option<Alarm0>>> = Mutex::new(RefCell::new(None));

// 16ns for serial clock cycle (write), page 43 of https://www.waveshare.com/w/upload/a/ae/ST7789_Datasheet.pdf
const SPI_ST7789VW_MAX_FREQ: embedded_time::rate::Hertz = embedded_time::rate::Hertz(62_500_000u32);

pub fn init() {
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

    unsafe { ALLOCATOR.init(&mut HEAP as *const u8 as usize, core::mem::size_of_val(&HEAP)) }

    let sio = hal::sio::Sio::new(pac.SIO);
    let pins = rp_pico::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);

    let _spi_sclk = pins.gpio10.into_mode::<hal::gpio::FunctionSpi>();
    let _spi_mosi = pins.gpio11.into_mode::<hal::gpio::FunctionSpi>();
    let _spi_miso = pins.gpio12.into_mode::<hal::gpio::FunctionSpi>();

    let spi = hal::spi::Spi::<_, _, 8>::new(pac.SPI1);
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        SPI_ST7789VW_MAX_FREQ,
        &embedded_hal::spi::MODE_3,
    );
    let spi = singleton!(:shared_bus::BusManagerSimple<hal::Spi<hal::spi::Enabled,  pac::SPI1, 8>> = shared_bus::BusManagerSimple::new(spi)).unwrap();

    let rst = pins.gpio15.into_push_pull_output();

    let dc = pins.gpio8.into_push_pull_output();
    let cs = pins.gpio9.into_push_pull_output();

    let (dc_copy, cs_copy) =
        unsafe { (core::ptr::read(&dc as *const _), core::ptr::read(&cs as *const _)) };

    let di = display_interface_spi::SPIInterface::new(spi.acquire_spi(), dc, cs);

    const DISPLAY_SIZE: PhysicalSize = PhysicalSize::new(320, 240);
    let mut display =
        st7789::ST7789::new(di, rst, DISPLAY_SIZE.width as _, DISPLAY_SIZE.height as _);

    // Turn on backlight
    {
        let mut bl = pins.gpio13.into_push_pull_output();
        bl.set_low().unwrap();
        delay.delay_us(10_000);
        bl.set_high().unwrap();
    }

    display.init(&mut delay).unwrap();
    display.set_orientation(st7789::Orientation::Landscape).unwrap();

    let touch_irq = pins.gpio17.into_pull_up_input();
    touch_irq.set_interrupt_enabled(GpioInterrupt::LevelLow, true);

    cortex_m::interrupt::free(|cs| {
        IRQ_PIN.borrow(cs).replace(Some(touch_irq));
    });

    let touch =
        xpt2046::XPT2046::new(&IRQ_PIN, pins.gpio16.into_push_pull_output(), spi.acquire_spi())
            .unwrap();

    let mut timer = Timer::new(pac.TIMER, &mut pac.RESETS);
    let mut alarm0 = timer.alarm_0().unwrap();
    alarm0.enable_interrupt();

    cortex_m::interrupt::free(|cs| {
        ALARM0.borrow(cs).replace(Some(alarm0));
    });

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::IO_IRQ_BANK0);
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
    }

    let dma = pac.DMA.split(&mut pac.RESETS);
    // SAFETY: This is not safe :-(
    let stolen_spi = unsafe {
        hal::spi::Spi::<_, _, 8>::new(rp_pico::hal::pac::Peripherals::steal().SPI1).init(
            &mut pac.RESETS,
            clocks.peripheral_clock.freq(),
            SPI_ST7789VW_MAX_FREQ,
            &embedded_hal::spi::MODE_3,
        )
    };
    let pio = PioTransfer::Idle(
        dma.ch0,
        vec![Rgb565::default(); DISPLAY_SIZE.width as _].leak(),
        stolen_spi,
    );

    crate::init_with_display(PicoDevices {
        display,
        touch,
        last_touch: Default::default(),
        timer,
        buffer: vec![Rgb565::default(); DISPLAY_SIZE.width as _].leak(),
        pio: Some(pio),
        stolen_pin: (dc_copy, cs_copy),
    });
}

enum PioTransfer<TO: WriteTarget, CH: SingleChannel> {
    Idle(CH, &'static mut [super::TargetPixel], TO),
    Running(hal::dma::SingleBuffering<CH, PartialReadBuffer, TO>),
}

impl<TO: WriteTarget<TransmittedWord = u8> + FullDuplex<u8>, CH: SingleChannel>
    PioTransfer<TO, CH>
{
    fn wait(self) -> (CH, &'static mut [super::TargetPixel], TO) {
        match self {
            PioTransfer::Idle(a, b, c) => (a, b, c),
            PioTransfer::Running(dma) => {
                let (a, b, mut to) = dma.wait();
                // After the DMA operated, we need to empty the receive FIFO, otherwise the touch screen
                // driver will pick wrong values. Continue to read as long as we don't get a Err(WouldBlock)
                while !to.read().is_err() {}
                (a, b.0, to)
            }
        }
    }
}

struct PicoDevices<Display, Touch, PioTransfer, Stolen> {
    display: Display,
    touch: Touch,
    last_touch: Option<i_slint_core::graphics::Point>,
    timer: Timer,
    buffer: &'static mut [super::TargetPixel],
    pio: Option<PioTransfer>,
    stolen_pin: Stolen,
}

impl<
        DI: display_interface::WriteOnlyDataCommand,
        RST: OutputPin<Error = Infallible>,
        IRQ: InputPin<Error = Infallible>,
        CS: OutputPin<Error = Infallible>,
        SPI: Transfer<u8>,
        TO: WriteTarget<TransmittedWord = u8> + FullDuplex<u8>,
        CH: SingleChannel,
        DC_: OutputPin<Error = Infallible>,
        CS_: OutputPin<Error = Infallible>,
    > Devices
    for PicoDevices<
        st7789::ST7789<DI, RST>,
        xpt2046::XPT2046<IRQ, CS, SPI>,
        PioTransfer<TO, CH>,
        (DC_, CS_),
    >
{
    fn screen_size(&self) -> PhysicalSize {
        let s = self.display.size();
        euclid::size2(s.width as _, s.height as _)
    }

    fn render_line(
        &mut self,
        line: PhysicalLength,
        dirty_region: crate::renderer::DirtyRegion,
        fill_buffer: &mut dyn FnMut(&mut [Rgb565]),
    ) {
        fill_buffer(self.buffer);

        for x in &mut self.buffer[dirty_region.min_x() as _..dirty_region.max_x() as _] {
            *x = embedded_graphics::pixelcolor::raw::RawU16::from(x.into_storage().to_be()).into()
        }
        let (ch, mut b, spi) = self.pio.take().unwrap().wait();
        self.stolen_pin.1.set_high().unwrap();

        /*self.display.set_pixels(
            dirty_region.min_x() as _,
            line.get() as _,
            dirty_region.max_x() as u16,
            line.get() as u16,
            self.buffer[dirty_region.origin.x as usize
                ..dirty_region.origin.x as usize + dirty_region.size.width as usize]
                .iter()
                .map(|x| embedded_graphics::pixelcolor::raw::RawU16::from(*x).into_inner()),
        );*/

        core::mem::swap(&mut self.buffer, &mut b);

        // We send empty data just to get the device in the right window
        self.display
            .set_pixels(
                dirty_region.min_x() as _,
                line.get() as _,
                dirty_region.max_x() as u16,
                line.get() as u16,
                core::iter::empty(),
            )
            .unwrap();

        self.stolen_pin.1.set_low().unwrap();
        self.stolen_pin.0.set_high().unwrap();
        let mut dma = hal::dma::SingleBufferingConfig::new(
            ch,
            PartialReadBuffer(b, dirty_region.min_x() as _..dirty_region.max_x() as _),
            spi,
        );
        dma.pace(hal::dma::Pace::PreferSink);
        self.pio = Some(PioTransfer::Running(dma.start()));
        /*let (a, b, c) = dma.start().wait();
        self.pio = Some(PioTransfer::Idle(a, b.0, c));*/
    }

    fn flush_frame(&mut self) {
        let (ch, b, spi) = self.pio.take().unwrap().wait();
        self.pio = Some(PioTransfer::Idle(ch, b, spi));
        self.stolen_pin.1.set_high().unwrap();
    }

    fn debug(&mut self, text: &str) {
        self.display.debug(text)
    }

    fn read_touch_event(&mut self) -> Option<i_slint_core::input::MouseEvent> {
        let button = i_slint_core::items::PointerEventButton::left;
        self.touch
            .read()
            .map_err(|_| ())
            .unwrap()
            .map(|point| {
                let size = self.screen_size().to_f32();
                let pos = euclid::point2(point.x * size.width, point.y * size.height).cast();
                self.display.fill_region(
                    crate::PhysicalRect::new(
                        crate::PhysicalPoint::from_untyped(pos.cast()),
                        euclid::size2(1, 1),
                    ),
                    &[embedded_graphics::prelude::RgbColor::RED],
                );
                match self.last_touch.replace(pos) {
                    Some(_) => i_slint_core::input::MouseEvent::MouseMoved { pos },
                    None => i_slint_core::input::MouseEvent::MousePressed { pos, button },
                }
            })
            .or_else(|| {
                self.last_touch
                    .take()
                    .map(|pos| i_slint_core::input::MouseEvent::MouseReleased { pos, button })
            })
    }

    fn time(&self) -> core::time::Duration {
        core::time::Duration::from_micros(self.timer.get_counter())
    }

    fn sleep(&self, duration: Option<core::time::Duration>) {
        let duration = duration.map(|d| {
            let d = core::cmp::max(d, core::time::Duration::from_micros(10));
            embedded_time::duration::Microseconds::new(d.as_micros() as u32)
        });

        cortex_m::interrupt::free(|cs| {
            if let Some(duration) = duration {
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

struct PartialReadBuffer(&'static mut [Rgb565], core::ops::Range<usize>);
unsafe impl embedded_dma::ReadBuffer for PartialReadBuffer {
    type Word = u8;

    unsafe fn read_buffer(&self) -> (*const <Self as embedded_dma::ReadBuffer>::Word, usize) {
        let act_slice = &self.0[self.1.clone()];
        (act_slice.as_ptr() as *const u8, act_slice.len() * core::mem::size_of::<Rgb565>())
    }
}

mod xpt2046 {
    use core::cell::RefCell;
    use cortex_m::interrupt::Mutex;
    use embedded_hal::blocking::spi::Transfer;
    use embedded_hal::digital::v2::{InputPin, OutputPin};
    use embedded_time::rate::Extensions;
    use euclid::default::Point2D;

    pub struct XPT2046<IRQ: InputPin + 'static, CS: OutputPin, SPI: Transfer<u8>> {
        irq: &'static Mutex<RefCell<Option<IRQ>>>,
        cs: CS,
        spi: SPI,
        pressed: bool,
    }

    impl<PinE, IRQ: InputPin<Error = PinE>, CS: OutputPin<Error = PinE>, SPI: Transfer<u8>>
        XPT2046<IRQ, CS, SPI>
    {
        pub fn new(
            irq: &'static Mutex<RefCell<Option<IRQ>>>,
            mut cs: CS,
            spi: SPI,
        ) -> Result<Self, PinE> {
            cs.set_high()?;
            Ok(Self { irq, cs, spi, pressed: false })
        }

        pub fn read(&mut self) -> Result<Option<Point2D<f32>>, Error<PinE, SPI::Error>> {
            const PRESS_THRESHOLD: i32 = -25_000;
            const RELEASE_THRESHOLD: i32 = -30_000;
            let threshold = if self.pressed { RELEASE_THRESHOLD } else { PRESS_THRESHOLD };
            self.pressed = false;

            if cortex_m::interrupt::free(|cs| {
                self.irq.borrow(cs).borrow().as_ref().unwrap().is_low()
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

                // FIXME! how else set the frequency to this device
                unsafe { set_spi_freq(3_000_000u32.Hz()) };

                self.cs.set_low().map_err(|e| Error::Pin(e))?;

                macro_rules! xchg {
                    ($byte:expr) => {
                        match self
                            .spi
                            .transfer(&mut [$byte, 0, 0])
                            .map_err(|e| Error::Transfer(e))?
                        {
                            [_, h, l] => ((*h as u32) << 8) | (*l as u32),
                            _ => return Err(Error::InternalError),
                        }
                    };
                }

                let z1 = xchg!(CMD_Z1_READ);
                let z2 = xchg!(CMD_Z2_READ);
                let z = z1 as i32 - z2 as i32;

                if z < threshold {
                    xchg!(0);
                    self.cs.set_high().map_err(|e| Error::Pin(e))?;
                    unsafe { set_spi_freq(super::SPI_ST7789VW_MAX_FREQ) };
                    return Ok(None);
                }

                xchg!(CMD_X_READ | 1); // Dummy read, first read is a outlier

                let mut point = Point2D::new(0u32, 0u32);
                for _ in 0..10 {
                    let y = xchg!(CMD_Y_READ);
                    let x = xchg!(CMD_X_READ);
                    point += euclid::vec2(i16::MAX as u32 - x, y)
                }

                let z1 = xchg!(CMD_Z1_READ);
                let z2 = xchg!(CMD_Z2_READ);
                let z = z1 as i32 - z2 as i32;

                xchg!(0);
                self.cs.set_high().map_err(|e| Error::Pin(e))?;
                unsafe { set_spi_freq(super::SPI_ST7789VW_MAX_FREQ) };

                if z < RELEASE_THRESHOLD {
                    return Ok(None);
                }

                point /= 10;
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
        InternalError,
    }

    unsafe fn set_spi_freq(freq: impl Into<super::Hertz<u32>>) {
        use rp_pico::hal;
        // FIXME: the touchscreen and the LCD have different frequencies, but we cannot really set different frequencies to different SpiProxy without this hack
        hal::spi::Spi::<_, _, 8>::new(hal::pac::Peripherals::steal().SPI1)
            .set_baudrate(125_000_000u32.Hz(), freq);
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
    let (mut pac, core) = unsafe { (pac::Peripherals::steal(), pac::CorePeripherals::steal()) };

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

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().integer());

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

    use core::fmt::Write;
    use embedded_graphics::{
        mono_font::{ascii::FONT_6X10, MonoTextStyle},
        prelude::*,
        text::Text,
    };

    display.init(&mut delay).unwrap();
    display.set_orientation(st7789::Orientation::Landscape).unwrap();
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
        delay.delay_ms(100);
        led.set_low().unwrap();
        delay.delay_ms(100);
        led.set_high().unwrap();
    }
}
