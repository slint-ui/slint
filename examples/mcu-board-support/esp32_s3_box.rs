// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::mem::MaybeUninit;
use embedded_graphics_core::geometry::OriginDimensions;
use embedded_hal::digital::OutputPin;
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
use mipidsi::{options::Orientation, Display};
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

        let i2c = i2c::I2c::new(
            peripherals.I2C0,
            i2c::Config { frequency: 400u32.kHz(), ..i2c::Config::default() },
        )
        .with_sda(peripherals.GPIO8)
        .with_scl(peripherals.GPIO18);

        let mut touch = tt21100::TT21100::new(i2c, Input::new(peripherals.GPIO3, Pull::Up))
            .expect("Initialize the touch device");

        let spi = spi::Spi::new_with_config(
            peripherals.SPI2,
            spi::Config { frequency: 60u32.MHz(), ..spi::Config::default() },
        )
        .with_sck(peripherals.GPIO7)
        .with_mosi(peripherals.GPIO6);

        let dc = Output::new(peripherals.GPIO4, Level::Low);
        let cs = Output::new(peripherals.GPIO5, Level::Low);
        let rst = Output::new(peripherals.GPIO48, Level::Low);

        let spi = embedded_hal_bus::spi::ExclusiveDevice::new_no_delay(spi, cs).unwrap();
        let mut buffer = [0u8; 512];
        let di = mipidsi::interface::SpiInterface::new(spi, dc, &mut buffer);
        let display = mipidsi::Builder::new(mipidsi::models::ILI9342CRgb565, di)
            .reset_pin(rst)
            .orientation(Orientation::new().rotate(mipidsi::options::Rotation::Deg180))
            .color_order(mipidsi::options::ColorOrder::Bgr)
            .init(&mut delay)
            .unwrap();

        let mut backlight = Output::new(peripherals.GPIO45, Level::High);
        backlight.set_high();

        let size = display.size();
        let size = slint::PhysicalSize::new(size.width, size.height);

        self.window.borrow().as_ref().unwrap().set_size(size);

        let mut buffer_provider = DrawBuffer {
            display,
            buffer: &mut [slint::platform::software_renderer::Rgb565Pixel(0); 320],
        };

        let mut last_touch = None;

        loop {
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                let mut event_count = 0;
                // The hardware keeps a queue of events. We should ideally process all event from the queue before rendering
                // or we will get outdated event in the next frames. But move events are constantly added to the queue
                // so we would block the whole interface, so add an arbitrary threshold
                while event_count < 15 && touch.data_available().unwrap() {
                    event_count += 1;
                    match touch.event() {
                        // Ignore error because we sometimes get an error at the beginning
                        Err(_) => (),
                        Ok(tt21100::Event::Button(..)) => (),
                        Ok(tt21100::Event::Touch { report: _, touches }) => {
                            let button = slint::platform::PointerEventButton::Left;
                            if let Some(event) = touches
                                .0
                                .map(|record| {
                                    let position = slint::PhysicalPosition::new(
                                        ((319. - record.x as f32) * size.width as f32 / 319.) as _,
                                        (record.y as f32 * size.height as f32 / 239.) as _,
                                    )
                                    .to_logical(window.scale_factor());
                                    match last_touch.replace(position) {
                                        Some(_) => WindowEvent::PointerMoved { position },
                                        None => WindowEvent::PointerPressed { position, button },
                                    }
                                })
                                .or_else(|| {
                                    last_touch.take().map(|position| WindowEvent::PointerReleased {
                                        position,
                                        button,
                                    })
                                })
                            {
                                let is_pointer_release_event =
                                    matches!(event, WindowEvent::PointerReleased { .. });

                                window.try_dispatch_event(event)?;

                                // removes hover state on widgets
                                if is_pointer_release_event {
                                    window.try_dispatch_event(WindowEvent::PointerExited)?;
                                }
                            }
                        }
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

impl<
        DI: mipidsi::interface::Interface<Word = u8>,
        RST: OutputPin<Error = core::convert::Infallible>,
    > slint::platform::software_renderer::LineBufferProvider
    for &mut DrawBuffer<'_, Display<DI, mipidsi::models::ILI9342CRgb565, RST>>
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

        // We send empty data just to get the device in the right window
        self.display
            .set_pixels(
                range.start as u16,
                line as _,
                range.end as u16,
                line as u16,
                buffer
                    .iter()
                    .map(|x| embedded_graphics_core::pixelcolor::raw::RawU16::new(x.0).into()),
            )
            .unwrap();
    }
}
