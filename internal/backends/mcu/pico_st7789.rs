// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

extern crate alloc;

use alloc::boxed::Box;
pub use cortex_m_rt::entry;
use embedded_hal::blocking::spi::Transfer;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use embedded_time::rate::*;
use rp_pico::hal::pac;
use rp_pico::hal::prelude::*;
use rp_pico::hal::{self, Timer};

use defmt_rtt as _; // global logger

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[alloc_error_handler]
fn oom(layout: core::alloc::Layout) -> ! {
    panic!("Out of memory {:?}", layout);
}
use alloc_cortex_m::CortexMHeap;

use crate::Devices;

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
        18_000_000u32.Hz(),
        &embedded_hal::spi::MODE_3,
    );
    // FIXME: a cleaner way to get a static reference, or be able to use non-static backend
    let spi = Box::leak(Box::new(shared_bus::BusManagerSimple::new(spi)));

    let rst = pins.gpio15.into_push_pull_output();

    let dc = pins.gpio8.into_push_pull_output();
    let cs = pins.gpio9.into_push_pull_output();
    let di = display_interface_spi::SPIInterface::new(spi.acquire_spi(), dc, cs);

    let mut display = st7789::ST7789::new(di, rst, 320, 240);

    // Turn on backlight
    {
        let mut bl = pins.gpio13.into_push_pull_output();
        bl.set_low().unwrap();
        delay.delay_us(10_000);
        bl.set_high().unwrap();
    }

    display.init(&mut delay).unwrap();
    display.set_orientation(st7789::Orientation::Landscape).unwrap();

    let touch = xpt2046::XPT2046::new(
        pins.gpio17.into_pull_down_input(),
        pins.gpio16.into_push_pull_output(),
        spi.acquire_spi(),
    )
    .unwrap();

    let timer = Timer::new(pac.TIMER, &mut pac.RESETS);

    crate::init_with_display(PicoDevices { display, touch, last_touch: Default::default(), timer });
}

struct PicoDevices<Display, Touch> {
    display: Display,
    touch: Touch,
    last_touch: Option<slint_core_internal::graphics::Point>,
    timer: Timer,
}

impl<Display: Devices, IRQ: InputPin, CS: OutputPin<Error = IRQ::Error>, SPI: Transfer<u8>> Devices
    for PicoDevices<Display, xpt2046::XPT2046<IRQ, CS, SPI>>
{
    fn screen_size(&self) -> slint_core_internal::graphics::IntSize {
        self.display.screen_size()
    }

    fn fill_region(
        &mut self,
        region: slint_core_internal::graphics::IntRect,
        pixels: &[embedded_graphics::pixelcolor::Rgb888],
    ) {
        self.display.fill_region(region, pixels)
    }

    fn debug(&mut self, text: &str) {
        self.display.debug(text)
    }

    fn read_touch_event(&mut self) -> Option<slint_core_internal::input::MouseEvent> {
        let button = slint_core_internal::items::PointerEventButton::left;
        self.touch
            .read()
            .map_err(|_| ())
            .unwrap()
            .map(|point| {
                let point = point.to_f32() / (i16::MAX as f32);
                let size = self.display.screen_size().to_f32();
                let pos = euclid::point2(point.x * size.width, point.y * size.height);
                match self.last_touch.replace(pos) {
                    Some(_) => slint_core_internal::input::MouseEvent::MouseMoved { pos },
                    None => slint_core_internal::input::MouseEvent::MousePressed { pos, button },
                }
            })
            .or_else(|| {
                self.last_touch.take().map(|pos| {
                    slint_core_internal::input::MouseEvent::MouseReleased { pos, button }
                })
            })
    }

    fn time(&mut self) -> core::time::Duration {
        core::time::Duration::from_micros(self.timer.get_counter())
    }
}

mod xpt2046 {
    use embedded_hal::blocking::spi::Transfer;
    use embedded_hal::digital::v2::{InputPin, OutputPin};
    use embedded_time::rate::Extensions;
    use euclid::default::Point2D;

    pub struct XPT2046<IRQ: InputPin, CS: OutputPin, SPI: Transfer<u8>> {
        irq: IRQ,
        cs: CS,
        spi: SPI,
    }

    impl<PinE, IRQ: InputPin<Error = PinE>, CS: OutputPin<Error = PinE>, SPI: Transfer<u8>>
        XPT2046<IRQ, CS, SPI>
    {
        pub fn new(irq: IRQ, mut cs: CS, spi: SPI) -> Result<Self, PinE> {
            cs.set_high()?;
            Ok(Self { irq, cs, spi })
        }

        pub fn read(&mut self) -> Result<Option<Point2D<i16>>, Error<PinE, SPI::Error>> {
            if self.irq.is_low().map_err(|e| Error::Pin(e))? {
                const CMD_X_READ: u8 = 0b10010000;
                const CMD_Y_READ: u8 = 0b11010000;

                let mut point = Point2D::new(0u32, 0u32);

                // FIXME! how else set the frequency to this device
                unsafe { set_spi_freq(3_000_000u32.Hz()) };

                self.cs.set_low().map_err(|e| Error::Pin(e))?;

                macro_rules! xchg {
                    ($byte:expr) => {
                        match self.spi.transfer(&mut [$byte]).map_err(|e| Error::Transfer(e))? {
                            [x] => *x as u32,
                            _ => return Err(Error::InternalError),
                        }
                    };
                }

                for _ in 0..10 {
                    xchg!(CMD_X_READ);
                    let mut x = xchg!(0) << 8;
                    x |= xchg!(CMD_Y_READ);
                    let mut y = xchg!(0) << 8;
                    y |= xchg!(0);

                    point += euclid::vec2(i16::MAX as u32 - x, y)
                }
                self.cs.set_high().map_err(|e| Error::Pin(e))?;

                unsafe { set_spi_freq(18_000_000u32.Hz()) };

                Ok(Some((point / 10).cast()))
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
        // FIXME: the touchscreen and the LCD have different frequencies, but we cannot really set different frequencies to different SpiProxy without this hack
        rp_pico::hal::spi::Spi::<_, _, 8>::new(rp_pico::hal::pac::Peripherals::steal().SPI1)
            .set_baudrate(125_000_000u32.Hz(), freq);
    }
}

#[cfg(not(feature = "panic-probe"))]
#[inline(never)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Safety: it's ok to steal here since we are in the manic handler, and the rest of the code will not be run anymore
    let (mut pac, core) = unsafe { (pac::Peripherals::steal(), pac::CorePeripherals::steal()) };

    let sio = hal::sio::Sio::new(pac.SIO);

    let pins = rp_pico::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);
    let mut led = pins.led.into_push_pull_output();
    led.set_high().unwrap();

    // Reset the heap so we can allocate the format string
    unsafe { ALLOCATOR.init(&mut HEAP as *const u8 as usize, core::mem::size_of_val(&HEAP)) }

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

    display.init(&mut delay).unwrap();
    display.set_orientation(st7789::Orientation::Landscape).unwrap();
    display
        .fill_solid(&display.bounding_box(), embedded_graphics::pixelcolor::Rgb565::WHITE)
        .unwrap();
    use embedded_graphics::{
        draw_target::DrawTarget,
        mono_font::{ascii::FONT_6X10, MonoTextStyle},
        prelude::*,
        text::Text,
    };
    let style = MonoTextStyle::new(&FONT_6X10, embedded_graphics::pixelcolor::Rgb565::RED);
    let mut y = 1;
    let mut x = 0;
    let width = display.bounding_box().size.width / 6 - 2;
    for line in alloc::format!("{}", info).split_inclusive(|c| {
        if c == '\n' || x > width {
            x = 0;
            true
        } else {
            x += 1;
            false
        }
    }) {
        Text::new(line, Point::new(0, y * 10), style).draw(&mut display).unwrap();
        y += 1;
    }

    loop {
        delay.delay_ms(100);
        led.set_low().unwrap();
        delay.delay_ms(100);
        led.set_high().unwrap();
    }
}
