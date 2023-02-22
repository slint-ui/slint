// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::geometry::OriginDimensions;
use embedded_hal::digital::v2::OutputPin;
use esp32s3_hal::{
    clock::{ClockControl, CpuClock},
    i2c::I2C,
    peripherals::Peripherals,
    prelude::*,
    spi::{Spi, SpiMode},
    systimer::SystemTimer,
    timer::TimerGroup,
    Delay, Rtc, IO,
};
use esp_alloc::EspHeap;
use esp_backtrace as _;
use mipidsi::{Display, Orientation};
use slint::platform::WindowEvent;
pub use xtensa_lx_rt::entry;

#[global_allocator]
static ALLOCATOR: EspHeap = EspHeap::empty();

pub fn init() {
    const HEAP_SIZE: usize = 250 * 1024;
    static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    unsafe { ALLOCATOR.init(&mut HEAP as *mut u8, core::mem::size_of_val(&HEAP)) }
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
            SystemTimer::now() / (SystemTimer::TICKS_PER_SECOND / 1000),
        )
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let peripherals = Peripherals::take();
        let mut system = peripherals.SYSTEM.split();
        let clocks = ClockControl::configure(system.clock_control, CpuClock::Clock240MHz).freeze();

        let mut rtc = Rtc::new(peripherals.RTC_CNTL);
        let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
        let mut wdt0 = timer_group0.wdt;
        let timer_group1 = TimerGroup::new(peripherals.TIMG1, &clocks);
        let mut wdt1 = timer_group1.wdt;

        rtc.rwdt.disable();
        wdt0.disable();
        wdt1.disable();

        let mut delay = Delay::new(&clocks);
        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

        let i2c = I2C::new(
            peripherals.I2C0,
            io.pins.gpio8,
            io.pins.gpio18,
            400u32.kHz(),
            &mut system.peripheral_clock_control,
            &clocks,
        );

        let mut touch = tt21100::TT21100::new(i2c, io.pins.gpio3.into_pull_up_input())
            .expect("Initialize the touch device");

        let sclk = io.pins.gpio7;
        let mosi = io.pins.gpio6;

        let spi = Spi::new_no_cs_no_miso(
            peripherals.SPI2,
            sclk,
            mosi,
            60u32.MHz(),
            SpiMode::Mode0,
            &mut system.peripheral_clock_control,
            &clocks,
        );

        let dc = io.pins.gpio4.into_push_pull_output();
        let rst = io.pins.gpio48.into_push_pull_output();

        let di = SPIInterfaceNoCS::new(spi, dc);
        let display = mipidsi::Builder::ili9342c_rgb565(di)
            .with_orientation(Orientation::PortraitInverted(false))
            .with_color_order(mipidsi::options::ColorOrder::Bgr)
            .init(&mut delay, Some(rst))
            .unwrap();

        let mut backlight = io.pins.gpio45.into_push_pull_output();
        backlight.set_high().unwrap();

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
                if touch.data_available().unwrap() {
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

                                window.dispatch_event(event);

                                // removes hover state on widgets
                                if is_pointer_release_event {
                                    window.dispatch_event(WindowEvent::PointerExited);
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
        DI: display_interface::WriteOnlyDataCommand,
        RST: OutputPin<Error = core::convert::Infallible>,
        MODEL: mipidsi::models::Model<ColorFormat = embedded_graphics::pixelcolor::Rgb565>,
    > slint::platform::software_renderer::LineBufferProvider
    for &mut DrawBuffer<'_, Display<DI, MODEL, RST>>
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
                buffer.iter().map(|x| embedded_graphics::pixelcolor::raw::RawU16::new(x.0).into()),
            )
            .unwrap();
    }
}

// FIXME: implement properly upstream
#[no_mangle]
extern "C" fn fmaxf(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}
#[no_mangle]
extern "C" fn fminf(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}
#[no_mangle]
extern "C" fn fmodf(a: f32, b: f32) -> f32 {
    ((a as u32) % (b as u32)) as f32
}
#[no_mangle]
extern "C" fn fmod(a: f64, b: f64) -> f64 {
    ((a as u32) % (b as u32)) as f64
}
