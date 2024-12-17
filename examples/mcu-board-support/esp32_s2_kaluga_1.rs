// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::{cell::RefCell, convert::Infallible, mem::MaybeUninit};
use display_interface_spi::SPIInterface;
use embedded_hal::digital::OutputPin;
use esp_alloc as _;
pub use esp_hal::entry;
use esp_hal::gpio::{Level, Output};
use esp_hal::spi::master as spi;
use esp_hal::timer::{systimer::SystemTimer, timg::TimerGroup};
use esp_hal::{delay::Delay, prelude::*, rtc_cntl::Rtc};
use esp_println::println;

type Display<DI, RST> = mipidsi::Display<DI, mipidsi::models::ST7789, RST>;

#[inline(never)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
    loop {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

pub fn init() {
    const HEAP_SIZE: usize = 160 * 1024;
    static mut HEAP: MaybeUninit<[u8; HEAP_SIZE]> = MaybeUninit::uninit();
    unsafe {
        esp_alloc::HEAP.add_region(esp_alloc::HeapRegion::new(
            HEAP.as_mut_ptr() as *mut u8,
            HEAP_SIZE,
            esp_alloc::MemoryCapability::Internal.into(),
        ));
    }
    slint::platform::set_platform(Box::new(EspBackend::default()))
        .expect("backend already initialized");
}

#[derive(Default)]
struct EspBackend {
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow>>>,
}

impl slint::platform::Platform for EspBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
            slint::platform::software_renderer::RepaintBufferType::ReusedBuffer,
        );
        self.window.replace(Some(window.clone()));
        Ok(window)
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(
            SystemTimer::now() / (SystemTimer::ticks_per_second() / 1000),
        )
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let peripherals = esp_hal::init(esp_hal::Config::default());

        // Disable the RTC and TIMG watchdog timers
        let mut rtc = Rtc::new(peripherals.LPWR);
        rtc.rwdt.disable();
        let mut timer_group0 = TimerGroup::new(peripherals.TIMG0);
        timer_group0.wdt.disable();
        let mut timer_group1 = TimerGroup::new(peripherals.TIMG1);
        timer_group1.wdt.disable();

        let mut delay = Delay::new();

        let mut backlight = Output::new(peripherals.GPIO6, Level::High);
        backlight.set_high();

        let mosi = peripherals.GPIO9;
        let cs = Output::new(peripherals.GPIO11, Level::Low);
        let rst = Output::new(peripherals.GPIO16, Level::Low);
        let dc = peripherals.GPIO13;
        let sck = peripherals.GPIO15;
        let miso = peripherals.GPIO8;

        let spi = spi::Spi::new_with_config(
            peripherals.SPI3,
            spi::Config { frequency: 80u32.MHz(), ..spi::Config::default() },
        )
        .with_sck(sck)
        .with_mosi(mosi)
        .with_miso(miso);
        let spi = embedded_hal_bus::spi::ExclusiveDevice::new_no_delay(spi, cs).unwrap();
        let di = SPIInterface::new(spi, Output::new(dc, Level::Low));
        let display = mipidsi::Builder::new(mipidsi::models::ST7789, di)
            .reset_pin(rst)
            .orientation(
                mipidsi::options::Orientation::new().rotate(mipidsi::options::Rotation::Deg90),
            )
            .init(&mut delay)
            .unwrap();

        let mut buffer_provider = DrawBuffer {
            display,
            buffer: &mut [slint::platform::software_renderer::Rgb565Pixel(0); 320],
        };

        self.window.borrow().as_ref().unwrap().set_size(slint::PhysicalSize::new(320, 240));

        loop {
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                window.draw_if_needed(|renderer| {
                    renderer.render_by_line(&mut buffer_provider);
                });
                if window.has_active_animations() {
                    continue;
                }
            }
            // TODO
        }
    }
}

struct DrawBuffer<'a, Display> {
    display: Display,
    buffer: &'a mut [slint::platform::software_renderer::Rgb565Pixel],
}

impl<DI: display_interface::WriteOnlyDataCommand, RST: OutputPin<Error = Infallible>>
    slint::platform::software_renderer::LineBufferProvider
    for &mut DrawBuffer<'_, Display<DI, RST>>
{
    type TargetPixel = slint::platform::software_renderer::Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [slint::platform::software_renderer::Rgb565Pixel]),
    ) {
        let buffer = &mut self.buffer[range.clone()];
        render_fn(buffer);
        self.display
            .set_pixels(
                range.start as u16,
                line as u16,
                range.end as u16,
                line as u16,
                buffer
                    .iter()
                    .map(|x| embedded_graphics_core::pixelcolor::raw::RawU16::new(x.0).into()),
            )
            .unwrap();
    }
}
