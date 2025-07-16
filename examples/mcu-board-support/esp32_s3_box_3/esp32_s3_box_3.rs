// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use embedded_graphics_core::geometry::OriginDimensions;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::DriveMode;
use esp_hal::peripherals::Peripherals;
use esp_hal::time::Instant;
use esp_hal::{
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    i2c::master::I2c,
    // init,
    spi::master::{Config as SpiConfig, Spi},
    spi::Mode as SpiMode,
    time::Rate,
};
use esp_println::logger::init_logger_from_env;
use gt911::Gt911Blocking;
use log::{error, info, warn};
use mipidsi::options::{ColorOrder, Orientation, Rotation};
use slint::platform::PointerEventButton;
use slint::platform::WindowEvent;

struct EspBackend {
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow>>>,
    peripherals: RefCell<Option<Peripherals>>,
}

impl Default for EspBackend {
    fn default() -> Self {
        EspBackend { window: RefCell::new(None), peripherals: RefCell::new(None) }
    }
}

/// Initializes the heap and sets the Slint platform.
pub fn init() {
    // Initialize peripherals first.
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_240MHz));
    init_logger_from_env();
    info!("Peripherals initialized");

    // Initialize the PSRAM allocator.
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);

    // Create an EspBackend that now owns the peripherals.
    slint::platform::set_platform(Box::new(EspBackend {
        peripherals: RefCell::new(Some(peripherals)),
        window: RefCell::new(None),
    }))
    .expect("backend already initialized");
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
        core::time::Duration::from_millis(Instant::now().duration_since_epoch().as_millis())
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        // Take and configure peripherals.
        let peripherals = self.peripherals.borrow_mut().take().expect("Peripherals already taken");
        let mut delay = Delay::new();

        // The following sequence is necessary to properly initialize touch on ESP32-S3-BOX-3
        // Based on issue from ESP-IDF: https://github.com/espressif/esp-bsp/issues/302#issuecomment-1971559689
        // Related code: https://github.com/espressif/esp-bsp/blob/30f0111a97b8fbe2efb7e58366fcf4d26b380f23/components/lcd_touch/esp_lcd_touch_gt911/esp_lcd_touch_gt911.c#L101-L133
        // --- Begin GT911 I²C Address Selection Sequence ---
        // Define constants for the two possible addresses.
        const ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS: u8 = 0x14;
        const ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS_BACKUP: u8 = 0x5D;

        // Our desired address.
        const DESIRED_ADDR: u8 = 0x14;
        // For desired address 0x14, assume the configuration’s reset level is false (i.e. 0 means active).
        let reset_level = Level::Low;

        // Configure the INT pin (GPIO3) as output; starting high because of internal pull-up.
        let mut int_pin = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
        // Force INT low to prepare for address selection.
        int_pin.set_low();
        delay.delay_ms(10);
        int_pin.set_low();
        delay.delay_ms(1);

        // Configure the shared RESET pin (GPIO48) as output in open–drain mode.
        let mut rst = Output::new(
            peripherals.GPIO48,
            Level::Low, // start in active state
            OutputConfig::default().with_drive_mode(DriveMode::OpenDrain),
        );

        // Set RESET to the reset-active level (here, false).
        rst.set_level(reset_level);
        // (Ensure INT remains low.)
        int_pin.set_low();
        delay.delay_ms(10);

        // Now, select the I²C address:
        // For GT911 address 0x14, the desired INT level is low; otherwise, for backup (0x5D) it would be high.
        let gpio_level = if DESIRED_ADDR == ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS {
            Level::Low
        } else if DESIRED_ADDR == ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS_BACKUP {
            Level::High
        } else {
            Level::Low
        };
        int_pin.set_level(gpio_level);
        delay.delay_ms(1);

        // Toggle the RESET pin:
        // Release RESET by setting it to the opposite of the reset level.
        rst.set_level(!reset_level);
        delay.delay_ms(10);
        delay.delay_ms(50);
        // --- End GT911 I²C Address Selection Sequence ---

        // --- Begin SPI and Display Initialization ---
        let spi = Spi::<esp_hal::Blocking>::new(
            peripherals.SPI2,
            SpiConfig::default().with_frequency(Rate::from_mhz(40)).with_mode(SpiMode::_0),
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
        let mut touch = Gt911Blocking::new(ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS);
        match touch.init(&mut i2c) {
            Ok(_) => info!("Touch initialized"),
            Err(e) => {
                warn!("Touch initialization failed: {:?}", e);
                let touch_fallback = Gt911Blocking::new(ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS_BACKUP);
                match touch_fallback.init(&mut i2c) {
                    Ok(_) => {
                        info!("Touch initialized with backup address");
                        touch = touch_fallback;
                    }
                    Err(e) => error!("Touch initialization failed with backup address: {:?}", e),
                }
            }
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
                buffer
                    .iter()
                    .map(|x| embedded_graphics_core::pixelcolor::raw::RawU16::new(x.0).into()),
            )
            .unwrap();
    }
}
