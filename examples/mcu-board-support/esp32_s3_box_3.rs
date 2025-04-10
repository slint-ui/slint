// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]

use embedded_hal::delay::DelayNs;
use esp_hal::peripherals::Peripherals;
use esp_hal::gpio::DriveMode;
use gt911::Gt911Blocking;
use log::{error, info};
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::time::Duration;
use embedded_graphics_core::geometry::OriginDimensions;
use embedded_hal::digital::OutputPin;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    delay::Delay,
    gpio::{Input, Level, Output, OutputConfig, Pull},
    i2c::master::I2c,
    // init,
    spi::master::{Spi, Config as SpiConfig},
    spi::Mode as SpiMode,
    timer::systimer::SystemTimer,
    time::Rate,
};
use mipidsi::{options::{Orientation, Rotation, ColorOrder}, Display};
use slint::platform::WindowEvent;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::gpio::InputConfig;
use esp_println::println;
use slint::platform::PointerEventButton;
use esp_hal::clock::CpuClock;

/// Initializes the heap and sets the Slint platform.
pub fn init() {
    println!("Initializing esp32");

    // Initialize peripherals first.
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_240MHz));
    println!("Peripherals initialized");

    // Initialize the PSRAM allocator.
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);

    // Create an EspBackend that now owns the peripherals.
    slint::platform::set_platform(Box::new(EspBackend {
        peripherals: RefCell::new(Some(peripherals)),
        window: RefCell::new(None),
        start_ticks: 0,
    }))
        .expect("backend already initialized");
}



#[derive(Default)]
struct EspBackend {
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow>>>,
    peripherals: RefCell<Option<Peripherals>>,
    start_ticks: u32,
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



    fn duration_since_start(&self) -> Duration {
        // let systimer = self.systimer.as_ref().expect("SystemTimer not initialized");
        let elapsed_ns = 10000;
        Duration::from_nanos(elapsed_ns)
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        // Take and configure peripherals.
        let mut peripherals = self
            .peripherals
            .borrow_mut()
            .take()
            .expect("Peripherals already taken");
        let mut delay = Delay::new();


        let mut rst = Output::new(
            peripherals.GPIO48,
            Level::High,
            OutputConfig::default().with_drive_mode(DriveMode::OpenDrain),
        );

        // --- Begin SPI and Display Initialization ---
        let spi = Spi::<esp_hal::Blocking>::new(
            peripherals.SPI2,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(40))
                .with_mode(SpiMode::_0),
        )
            .unwrap()
            .with_sck(peripherals.GPIO7)
            .with_mosi(peripherals.GPIO6);

        // Display control pins.
        let dc = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
        let cs = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());

        // Wrap SPI into a bus.
        let spi_delay = Delay::new();
        let spi_device = ExclusiveDevice::new(spi, cs, spi_delay).unwrap();
        let mut buffer = [0u8; 512];
        let di = mipidsi::interface::SpiInterface::new(spi_device, dc, &mut buffer);

        // Initialize the display.
        let display = mipidsi::Builder::new(mipidsi::models::ILI9486Rgb565, di)
            .reset_pin(rst)
            .orientation(Orientation::new().rotate(Rotation::Deg180))
            .color_order(ColorOrder::Bgr)
            .init(&mut delay)
            .unwrap();

        // Set up the backlight pin.
        let mut backlight = Output::new(peripherals.GPIO47, Level::Low, OutputConfig::default());
        backlight.set_high();

        // Update the Slint window size from the display.
        let size = display.size();
        let size = slint::PhysicalSize::new(size.width, size.height);
        self.window.borrow().as_ref().unwrap().set_size(size);

        // --- End Display Initialization ---

        let mut i2c = I2c::new(
            peripherals.I2C0,
            esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
        )
            .unwrap()
            .with_sda(peripherals.GPIO8)
            .with_scl(peripherals.GPIO18);

        // Initialize the touch driver.
        let mut touch = Gt911Blocking::new(0x14);
        match touch.init(&mut i2c) {
            Ok(_) => println!("Touch initialized"),
            Err(e) => println!("Touch initialization failed: {:?}", e),
        }

        // Prepare a draw buffer for the Slint software renderer.
        let mut buffer_provider = DrawBuffer {
            display,
            buffer: &mut [slint::platform::software_renderer::Rgb565Pixel(0); 320],
        };

        // Variable to track the last touch position.
        let mut last_touch = None;

        // Main event loop.
        loop {
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                // Poll the GT911 for touch data.
                match touch.get_touch(&mut i2c) {
                    // Active touch detected: Some(point) means a press or move.
                    Ok(Some(point)) => {
                        // Convert GT911 raw coordinates (assumed in pixels) into a PhysicalPosition.
                        let pos = slint::PhysicalPosition::new(point.x as i32, point.y as i32)
                            .to_logical(window.scale_factor());

                        let event = if let Some(previous_pos) = last_touch.replace(pos) {
                            // If the position changed, send a PointerMoved event.
                            if previous_pos != pos {
                                WindowEvent::PointerMoved { position: pos }
                            } else {
                                // If the position is unchanged, skip event generation.
                                continue;
                            }
                        } else {
                            // No previous touch recorded, generate a PointerPressed event.
                            WindowEvent::PointerPressed {
                                position: pos,
                                button: PointerEventButton::Left,
                            }
                        };

                        // Dispatch the event to Slint.
                        window.try_dispatch_event(event)?;
                    }
                    // No active touch: if a previous touch existed, dispatch pointer release.
                    Ok(None) => {
                        if let Some(pos) = last_touch.take() {
                            window.try_dispatch_event(WindowEvent::PointerReleased {
                                position: pos,
                                button: PointerEventButton::Left,
                            })?;
                            window.try_dispatch_event(WindowEvent::PointerExited)?;
                        }
                    }
                    // On errors, you can log them if desired.
                    Err(_) => {
                        // Optionally log or ignore errors.
                    }
                }

                // Render the window if needed.
                window.draw_if_needed(|renderer| {
                    renderer.render_by_line(&mut buffer_provider);
                });

                if window.has_active_animations() {
                    continue;
                }
            }
        }
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        esp_println::println!("{}", arguments);
    }
}

/// Provides a draw buffer for the MinimalSoftwareWindow renderer.
struct DrawBuffer<'a, Display> {
    display: Display,
    buffer: &'a mut [slint::platform::software_renderer::Rgb565Pixel],
}

impl<
    DI: mipidsi::interface::Interface<Word = u8>,
    RST: OutputPin<Error = core::convert::Infallible>,
> slint::platform::software_renderer::LineBufferProvider
for &mut DrawBuffer<'_, mipidsi::Display<DI, mipidsi::models::ILI9486Rgb565, RST>>
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

        // Update the display with the rendered line.
        self.display
            .set_pixels(
                range.start as u16,
                line as u16,
                range.end as u16,
                line as u16,
                buffer.iter().map(|x| embedded_graphics_core::pixelcolor::raw::RawU16::new(x.0).into()),
            )
            .unwrap();
    }
}
