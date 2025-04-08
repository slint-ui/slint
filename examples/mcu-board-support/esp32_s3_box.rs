// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]

use esp_hal::peripherals::Peripherals;
use esp_hal::gpio::DriveMode;

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

/// Initializes the heap and sets the Slint platform.
pub fn init() {
    println!("Initializing esp32");

    // Initialize peripherals first.
    let peripherals = esp_hal::init(esp_hal::Config::default());
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
        // Initialize the peripherals using the new esp-hal 1.0.0 API.
        let mut peripherals = self
            .peripherals
            .borrow_mut()
            .take()
            .expect("Peripherals already taken");
        let mut delay = Delay::new();
        // I2C initialization.
        let i2c = I2c::new(
            peripherals.I2C0,
            esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
        )
            .unwrap()
            .with_sda(peripherals.GPIO8)
            .with_scl(peripherals.GPIO18);

        // Initialize the touch driver
        // let mut touch = tt21100::TT21100::new(i2c, Input::new(peripherals.GPIO3, InputConfig::default().with_pull(Pull::Up)))
        //     .expect("Initialize the touch device");

        // SPI initialization: using Blocking mode. Update the configuration to use Rate.
        let spi = Spi::<esp_hal::Blocking>::new(
            peripherals.SPI2,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(60))
                .with_mode(SpiMode::_0),
        )
            .unwrap()
            .with_sck(peripherals.GPIO7)
            .with_mosi(peripherals.GPIO6);

        // Configure display control and chip select pins with an output config.
        let dc = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
        let cs = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());
        // Reset pin for display: GPIO48 (OpenDrain required).
        let rst = Output::new(
            peripherals.GPIO48,
            Level::High,
            OutputConfig::default().with_drive_mode(DriveMode::OpenDrain),
        );

        // Wrap the SPI bus in an ExclusiveDevice.
        let spi_delay = Delay::new();
        let spi_device = ExclusiveDevice::new(spi, cs, spi_delay).unwrap();

        // Create a temporary buffer for the SPI interface.
        let mut buffer = [0u8; 512];
        let di = mipidsi::interface::SpiInterface::new(spi_device, dc, &mut buffer);

        // Initialize the display with updated builder settings.
        let display = mipidsi::Builder::new(mipidsi::models::ILI9486Rgb565, di)
            .reset_pin(rst)
            .orientation(Orientation::new().rotate(Rotation::Deg180))
            .color_order(ColorOrder::Bgr)
            .init(&mut delay)
            .unwrap();

        // Set up the backlight pin. Using an output config as required by the new API.
        let mut backlight = Output::new(peripherals.GPIO47, Level::Low, OutputConfig::default());
        backlight.set_high();

        // Use the display size to size the Slint window.
        let size = display.size();
        let size = slint::PhysicalSize::new(size.width, size.height);

        self.window
            .borrow()
            .as_ref()
            .unwrap()
            .set_size(size);

        // Prepare a draw buffer that is used by the Slint software renderer.
        let mut buffer_provider = DrawBuffer {
            display,
            buffer: &mut [slint::platform::software_renderer::Rgb565Pixel(0); 320],
        };

        // Variable to track touch state between iterations.
        // let mut last_touch = None;

        // Main event loop.
        loop {
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                let mut event_count = 0;
                // Process up to 15 events per frame so that we do not block rendering.
                // while event_count < 15 && touch.data_available().unwrap() {
                //     event_count += 1;
                //     match touch.event() {
                //         // Discard errors (e.g. transient initialization issues).
                //         Err(_) => (),
                //         Ok(tt21100::Event::Button(..)) => (),
                //         Ok(tt21100::Event::Touch { report: _, touches }) => {
                //             let button = slint::platform::PointerEventButton::Left;
                //             if let Some(event) = touches
                //                 .0
                //                 .map(|record| {
                //                     let position = slint::PhysicalPosition::new(
                //                         // Map coordinates from display to logical coordinates.
                //                         ((319. - record.x as f32) * size.width as f32 / 319.) as _,
                //                         (record.y as f32 * size.height as f32 / 239.) as _,
                //                     )
                //                         .to_logical(window.scale_factor());
                //                     match last_touch.replace(position) {
                //                         Some(_) => WindowEvent::PointerMoved { position },
                //                         None => WindowEvent::PointerPressed { position, button },
                //                     }
                //                 })
                //                 .or_else(|| {
                //                     last_touch.take().map(|position| WindowEvent::PointerReleased {
                //                         position,
                //                         button,
                //                     })
                //                 })
                //             {
                //                 let is_pointer_release_event =
                //                     matches!(event, WindowEvent::PointerReleased { .. });
                //
                //                 window.try_dispatch_event(event)?;
                //
                //                 // Remove hover state on widgets after a pointer release.
                //                 if is_pointer_release_event {
                //                     window.try_dispatch_event(WindowEvent::PointerExited)?;
                //                 }
                //             }
                //         }
                //     }
                // }

                window.draw_if_needed(|renderer| {
                    renderer.render_by_line(&mut buffer_provider);
                });
                if window.has_active_animations() {
                    continue;
                }
            }
            // TODO: Add sleep or power saving functionality if necessary.
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
