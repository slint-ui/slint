// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::{cell::RefCell, convert::Infallible};
use display_interface_spi::SPIInterfaceNoCS;
use embedded_hal::digital::v2::OutputPin;
use esp32s2_hal::{
    clock::ClockControl, peripherals::Peripherals, prelude::*, spi, systimer::SystemTimer,
    timer::TimerGroup, Delay, Rtc, IO,
};
use esp_alloc::EspHeap;
use esp_println::println;
pub use xtensa_lx_rt::entry;

#[inline(never)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
    loop {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

#[global_allocator]
static ALLOCATOR: EspHeap = EspHeap::empty();

pub fn init() {
    const HEAP_SIZE: usize = 160 * 1024;
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
        let mut clocks = ClockControl::boot_defaults(system.clock_control).freeze();

        // Disable the RTC and TIMG watchdog timers
        let mut rtc_cntl = Rtc::new(peripherals.RTC_CNTL);
        let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
        let mut wdt0 = timer_group0.wdt;
        let timer_group1 = TimerGroup::new(peripherals.TIMG1, &clocks);
        let mut wdt1 = timer_group1.wdt;

        rtc_cntl.rwdt.disable();
        wdt0.disable();
        wdt1.disable();

        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
        let backlight = io.pins.gpio6;
        let mut backlight = backlight.into_push_pull_output();
        backlight.set_high().unwrap();

        let mosi = io.pins.gpio9;
        let cs = io.pins.gpio11;
        let rst = io.pins.gpio16;
        let dc = io.pins.gpio13;
        let sck = io.pins.gpio15;
        let miso = io.pins.gpio8;

        let spi = spi::Spi::new(
            peripherals.SPI3,
            sck,
            mosi,
            miso,
            cs,
            80u32.MHz(),
            spi::SpiMode::Mode0,
            &mut system.peripheral_clock_control,
            &mut clocks,
        );

        let di = SPIInterfaceNoCS::new(spi, dc.into_push_pull_output());
        let reset = rst.into_push_pull_output();
        let mut display = st7789::ST7789::new(di, Some(reset), Some(backlight), 320, 240);
        let mut delay = Delay::new(&clocks);

        display.init(&mut delay).unwrap();
        display.set_orientation(st7789::Orientation::Landscape).unwrap();
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

impl<
        DI: display_interface::WriteOnlyDataCommand,
        RST: OutputPin<Error = Infallible>,
        BL: OutputPin<Error = Infallible>,
    > slint::platform::software_renderer::LineBufferProvider
    for &mut DrawBuffer<'_, st7789::ST7789<DI, RST, BL>>
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
                buffer.iter().map(|x| !x.0),
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
