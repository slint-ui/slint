// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! https://www.waveshare.com/wiki/ESP32-S3-Touch-LCD-1.28

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::mem::MaybeUninit;
use display_interface_spi::SPIInterface;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::delay::Delay;
pub use esp_hal::entry;
use esp_hal::gpio::{Input, Level, Output, Pull};
use esp_hal::i2c::master as i2c;
use esp_hal::prelude::*;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::spi::master as spi;
use esp_hal::timer::{systimer::SystemTimer, timg::TimerGroup};
use gc9a01::{prelude::*, Gc9a01};
use slint::platform::WindowEvent;

pub fn init() {
    const HEAP_SIZE: usize = 250 * 1024;
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

        let mut rtc = Rtc::new(peripherals.LPWR);
        rtc.rwdt.disable();
        let mut timer_group0 = TimerGroup::new(peripherals.TIMG0);
        timer_group0.wdt.disable();
        let mut timer_group1 = TimerGroup::new(peripherals.TIMG1);
        timer_group1.wdt.disable();

        let mut delay = Delay::new();

        let i2c = i2c::I2c::new(peripherals.I2C0, i2c::Config::default())
            .with_sda(peripherals.GPIO6)
            .with_scl(peripherals.GPIO7);

        let touch_rst = Output::new(peripherals.GPIO13, Level::Low);
        let touch_int = Input::new(peripherals.GPIO5, Pull::Up);

        let mut touch = cst816s::CST816S::new(i2c, touch_int, touch_rst);
        touch.setup(&mut delay).unwrap();

        let spi = spi::Spi::new_with_config(
            peripherals.SPI2,
            spi::Config { frequency: 40u32.MHz(), ..spi::Config::default() },
        )
        .with_sck(peripherals.GPIO10)
        .with_mosi(peripherals.GPIO11)
        .with_miso(peripherals.GPIO12);

        let dc = Output::new(peripherals.GPIO8, Level::Low);
        let cs = Output::new(peripherals.GPIO9, Level::Low);
        let mut rst = Output::new(peripherals.GPIO14, Level::Low);

        let spi = embedded_hal_bus::spi::ExclusiveDevice::new_no_delay(spi, cs).unwrap();

        let di = SPIInterface::new(spi, dc);
        let mut display = Gc9a01::new(di, DisplayResolution240x240, DisplayRotation::Rotate180);
        display.reset(&mut rst, &mut delay).unwrap();
        display.init(&mut delay).unwrap();

        let mut backlight = Output::new(peripherals.GPIO2, Level::High);
        backlight.set_high();

        let size = display.dimensions();
        let size = slint::PhysicalSize::new(size.0 as _, size.1 as _);

        self.window.borrow().as_ref().unwrap().set_size(size);

        let mut buffer_provider = DrawBuffer {
            display,
            buffer: &mut [slint::platform::software_renderer::Rgb565Pixel(0); 320],
        };

        loop {
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                if let Some(evt) = touch.read_one_touch_event(true) {
                    let position = slint::PhysicalPosition::new(evt.x as _, evt.y as _)
                        .to_logical(window.scale_factor());
                    let button = slint::platform::PointerEventButton::Left;
                    match evt.action {
                        // down
                        0 => {
                            window.dispatch_event(WindowEvent::PointerPressed { position, button });
                        }
                        // up
                        1 => {
                            window
                                .dispatch_event(WindowEvent::PointerReleased { position, button });
                            window.dispatch_event(WindowEvent::PointerExited);
                        }
                        // move
                        2 => {
                            window.dispatch_event(WindowEvent::PointerMoved { position });
                        }
                        _ => {}
                    }
                }

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

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        esp_println::println!("{}", arguments);
    }
}

struct DrawBuffer<'a, Display> {
    display: Display,
    buffer: &'a mut [slint::platform::software_renderer::Rgb565Pixel],
}

impl<I: display_interface::WriteOnlyDataCommand, D: DisplayDefinition>
    slint::platform::software_renderer::LineBufferProvider
    for &mut DrawBuffer<'_, Gc9a01<I, D, gc9a01::mode::BasicMode>>
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
                (range.start as u16, line as u16),
                (range.end as u16, line as u16),
                &mut buffer.iter().map(|x| x.0),
            )
            .unwrap();
    }
}
