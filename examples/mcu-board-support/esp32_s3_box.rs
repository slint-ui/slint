// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use display_interface_spi::SPIInterfaceNoCS;
use embedded_hal::digital::v2::OutputPin;
use esp32s3_hal::{
    clock::ClockControl,
    pac::Peripherals,
    prelude::*,
    spi::{Spi, SpiMode},
    systimer::SystemTimer,
    timer::TimerGroup,
    Delay, Rtc, IO,
};
use esp_alloc::EspHeap;
use esp_backtrace as _;
use mipidsi::{Display, DisplayOptions, Orientation};
pub use xtensa_lx_rt::entry;

#[alloc_error_handler]
fn oom(layout: core::alloc::Layout) -> ! {
    panic!("Out of memory {:?}", layout);
}

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
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow<1>>>>,
}

impl slint::platform::Platform for EspBackend {
    fn create_window_adapter(&self) -> Rc<dyn slint::platform::WindowAdapter> {
        let window = slint::platform::software_renderer::MinimalSoftwareWindow::new();
        self.window.replace(Some(window.clone()));
        window
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(
            SystemTimer::now() / (SystemTimer::TICKS_PER_SECOND / 1000),
        )
    }

    fn run_event_loop(&self) {
        let peripherals = Peripherals::take().unwrap();
        let mut system = peripherals.SYSTEM.split();
        let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

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

        let sclk = io.pins.gpio7;
        let mosi = io.pins.gpio6;

        let spi = Spi::new_no_cs_no_miso(
            peripherals.SPI2,
            sclk,
            mosi,
            4u32.MHz(),
            SpiMode::Mode0,
            &mut system.peripheral_clock_control,
            &clocks,
        );

        let dc = io.pins.gpio4.into_push_pull_output();
        let rst = io.pins.gpio48.into_push_pull_output();

        let di = SPIInterfaceNoCS::new(spi, dc);
        let mut display = Display::ili9342c_rgb565(di, rst);

        display
            .init(
                &mut delay,
                DisplayOptions {
                    orientation: Orientation::PortraitInverted(false),
                    ..DisplayOptions::default()
                },
            )
            .unwrap();

        let mut backlight = io.pins.gpio45.into_push_pull_output();
        backlight.set_high().unwrap();

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
    for &mut DrawBuffer<'_, Display<DI, RST, MODEL>>
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
extern "C" fn fmodf() {
    unimplemented!("fmodf");
}
#[no_mangle]
extern "C" fn fmod(a: f64, b: f64) -> f64 {
    ((a as u32) % (b as u32)) as f64
}
