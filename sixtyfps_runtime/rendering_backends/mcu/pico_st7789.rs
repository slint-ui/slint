// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

extern crate alloc;

use alloc::boxed::Box;
pub use cortex_m_rt::entry;
use embedded_hal::blocking::spi::Transfer;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use embedded_time::rate::*;
use rp_pico::hal;
use rp_pico::hal::pac;
use rp_pico::hal::prelude::*;

use defmt_rtt as _; // global logger
use panic_probe as _;

#[alloc_error_handler]
fn oom(_: core::alloc::Layout) -> ! {
    loop {}
}
use alloc_cortex_m::CortexMHeap;
use rp_pico::hal::rtc::{DateTime, RealTimeClock};

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
        4_000_000u32.Hz(),
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

    let clock = RealTimeClock::new(
        pac.RTC,
        clocks.rtc_clock,
        &mut pac.RESETS,
        DateTime {
            year: 1970,
            month: 1,
            day: 1,
            day_of_week: hal::rtc::DayOfWeek::Thursday,
            hour: 0,
            minute: 0,
            second: 0,
        },
    )
    .unwrap();

    crate::init_with_display(PicoDevices { display, touch, last_touch: Default::default(), clock });
}

struct PicoDevices<Display, Touch> {
    display: Display,
    touch: Touch,
    last_touch: Option<sixtyfps_corelib::graphics::Point>,
    clock: RealTimeClock,
}

impl<Display: Devices, IRQ: InputPin, CS: OutputPin<Error = IRQ::Error>, SPI: Transfer<u8>> Devices
    for PicoDevices<Display, xpt2046::XPT2046<IRQ, CS, SPI>>
{
    fn screen_size(&self) -> sixtyfps_corelib::graphics::IntSize {
        self.display.screen_size()
    }

    fn fill_region(
        &mut self,
        region: sixtyfps_corelib::graphics::IntRect,
        pixels: &[embedded_graphics::pixelcolor::Rgb888],
    ) {
        self.display.fill_region(region, pixels)
    }

    fn debug(&mut self, text: &str) {
        self.display.debug(text)
    }

    fn read_touch_event(&mut self) -> Option<sixtyfps_corelib::input::MouseEvent> {
        let button = sixtyfps_corelib::items::PointerEventButton::left;
        self.touch
            .read()
            .map_err(|_| ())
            .unwrap()
            .map(|point| {
                let point = point.to_f32() / (i16::MAX as f32);
                let size = self.display.screen_size().to_f32();
                let pos = euclid::point2(point.x * size.width, point.y * size.height);
                match self.last_touch.replace(pos) {
                    Some(_) => sixtyfps_corelib::input::MouseEvent::MouseMoved { pos },
                    None => sixtyfps_corelib::input::MouseEvent::MousePressed { pos, button },
                }
            })
            .or_else(|| {
                self.last_touch
                    .take()
                    .map(|pos| sixtyfps_corelib::input::MouseEvent::MouseReleased { pos, button })
            })
    }

    fn time(&mut self) -> core::time::Duration {
        let time = self.clock.now();
        match time {
            // FIXME! milisecond
            Ok(t) => core::time::Duration::from_secs(t.second as u64 + t.minute as u64 * 60),
            Err(_) => core::time::Duration::ZERO,
        }
    }
}

mod xpt2046 {
    use embedded_hal::blocking::spi::Transfer;
    use embedded_hal::digital::v2::{InputPin, OutputPin};
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
                self.cs.set_low().map_err(|e| Error::Pin(e))?;
                const CMD_X_READ: u8 = 0b10010000;
                const CMD_Y_READ: u8 = 0b11010000;

                macro_rules! xchg {
                    ($byte:expr) => {
                        match self.spi.transfer(&mut [$byte]).map_err(|e| Error::Transfer(e))? {
                            [x] => *x as i16,
                            _ => return Err(Error::InternalError),
                        }
                    };
                }

                xchg!(CMD_X_READ);
                let mut x = xchg!(0) << 8;
                x += xchg!(CMD_Y_READ);
                let mut y = xchg!(0) << 8;
                y += xchg!(0);

                self.cs.set_high().map_err(|e| Error::Pin(e))?;

                Ok(Some(Point2D::new(i16::MAX - x, y)))
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
}
