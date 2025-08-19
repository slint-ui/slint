// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    esp_println::println!("Panic: {:?}", info);
    loop {}
}

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use embedded_graphics_core::draw_target::DrawTarget;
use embedded_graphics_core::geometry::OriginDimensions;
use embedded_graphics_core::pixelcolor::RgbColor;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::peripherals::Peripherals;
use esp_hal::time::Instant;
use esp_hal::{
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    i2c::master::I2c,
    spi::master::{Config as SpiConfig, Spi},
    spi::Mode as SpiMode,
    time::Rate,
};
use esp_println::logger::init_logger_from_env;
use log::{error, info};
use mipidsi::options::{ColorInversion, ColorOrder};

// Touch support imports
use embedded_hal_bus::i2c::RefCellDevice;
use ft3x68_rs::{Ft3x68Driver, ResetInterface};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::PhysicalPosition;
use static_cell::StaticCell;

// FT6336U I2C address (compatible with FT3x68 driver)
const FT6336U_DEVICE_ADDRESS: u8 = 0x38;

// AW9523 I2C address
const AW9523_I2C_ADDRESS: u8 = 0x58;

/// Touch reset implementation via AW9523 GPIO expander using direct I2C commands
/// Based on the AW9523 datasheet and M5Stack CoreS3 schematics
pub struct TouchResetDriverAW9523<I2C> {
    i2c: I2C,
}

impl<I2C> TouchResetDriverAW9523<I2C> {
    pub fn new(i2c: I2C) -> Self {
        TouchResetDriverAW9523 { i2c }
    }
}

impl<I2C> ResetInterface for TouchResetDriverAW9523<I2C>
where
    I2C: embedded_hal::i2c::I2c,
{
    type Error = I2C::Error;

    fn reset(&mut self) -> Result<(), Self::Error> {
        let delay = Delay::new();

        // AW9523 register addresses:
        // 0x02: Port 0 Configuration (0=output, 1=input)
        // 0x03: Port 1 Configuration (0=output, 1=input)
        // 0x04: Port 0 Output (pin values for outputs)
        // 0x05: Port 1 Output (pin values for outputs)

        // Configure P0_0 (touch reset) as output (bit 0 = 0)
        // Keep other pins as they are - read current config first
        let mut config_p0 = [0u8; 1];
        self.i2c.write_read(AW9523_I2C_ADDRESS, &[0x02], &mut config_p0)?;
        let new_config_p0 = config_p0[0] & !0x01; // Clear bit 0 to make P0_0 output
        self.i2c.write(AW9523_I2C_ADDRESS, &[0x02, new_config_p0])?;

        // Pull reset (P0_0) low
        let mut output_p0 = [0u8; 1];
        self.i2c.write_read(AW9523_I2C_ADDRESS, &[0x04], &mut output_p0)?;
        let new_output_low = output_p0[0] & !0x01; // Clear bit 0 to pull P0_0 low
        self.i2c.write(AW9523_I2C_ADDRESS, &[0x04, new_output_low])?;
        delay.delay_millis(10);

        // Pull reset (P0_0) high
        let new_output_high = output_p0[0] | 0x01; // Set bit 0 to pull P0_0 high
        self.i2c.write(AW9523_I2C_ADDRESS, &[0x04, new_output_high])?;
        delay.delay_millis(300);

        Ok(())
    }
}

struct EspBackend {
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow>>>,
    peripherals: RefCell<Option<Peripherals>>,
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
        self.run_event_loop()
    }
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

/// Initialize the AXP2101 power management unit for M5Stack CoreS3 using shared I2C
/// This implements the exact same sequence as the working custom implementation
/// Based on https://github.com/tuupola/axp192
/// and https://github.com/m5stack/M5CoreS3/blob/main/src/AXP2101.cpp
fn init_axp2101_power<I2C>(mut i2c_device: I2C) -> Result<(), ()>
where
    I2C: embedded_hal::i2c::I2c,
{
    const AXP2101_ADDRESS: u8 = 0x34;

    info!("Initializing AXP2101 power management with M5Stack CoreS3 sequence...");

    // This sequence matches exactly the working custom implementation:
    // 1. CHG_LED register (0x69) <- 0x35 (0b00110101)
    // 2. ALDO_ENABLE register (0x90) <- 0xBF
    // 3. ALDO4 register (0x95) <- 0x1C (0b00011100)

    // Step 1: Configure charge LED (register 0x69 = 105 decimal)
    if i2c_device.write(AXP2101_ADDRESS, &[0x69, 0x35]).is_err() {
        error!("Failed to write to CHG_LED register (0x69)");
        return Err(());
    }
    info!("AXP2101: CHG_LED configured (0x69 <- 0x35)");

    // Step 2: Enable ALDO outputs (register 0x90 = 144 decimal)
    if i2c_device.write(AXP2101_ADDRESS, &[0x90, 0xBF]).is_err() {
        error!("Failed to write to ALDO_ENABLE register (0x90)");
        return Err(());
    }
    info!("AXP2101: ALDO outputs enabled (0x90 <- 0xBF)");

    // Step 3: Configure ALDO4 voltage (register 0x95 = 149 decimal)
    if i2c_device.write(AXP2101_ADDRESS, &[0x95, 0x1C]).is_err() {
        error!("Failed to write to ALDO4 register (0x95)");
        return Err(());
    }
    info!("AXP2101: ALDO4 voltage configured (0x95 <- 0x1C)");

    info!("AXP2101 power management initialized successfully with M5Stack CoreS3 sequence");
    Ok(())
}

/// Initialize the AW9523 GPIO expander for M5Stack CoreS3 using shared I2C
/// This implements the exact same sequence as the working custom implementation
/// Based on: https://github.com/m5stack/M5CoreS3/blob/main/src/AXP2101.cpp
fn init_aw9523_gpio_expander<I2C>(mut i2c_device: I2C) -> Result<(), ()>
where
    I2C: embedded_hal::i2c::I2c,
{
    info!("Initializing AW9523 GPIO expander with M5Stack CoreS3 sequence...");

    // Step 1: Configure Port 0 Configuration (register 0x02) <- 0b00000101 (0x05)
    if i2c_device.write(AW9523_I2C_ADDRESS, &[0x02, 0b00000101]).is_err() {
        error!("Failed to write to AW9523 Port 0 Configuration register (0x02)");
        return Err(());
    }
    info!("AW9523: Port 0 Configuration set (0x02 <- 0x05)");

    // Step 2: Configure Port 1 Configuration (register 0x03) <- 0b00000011 (0x03)
    if i2c_device.write(AW9523_I2C_ADDRESS, &[0x03, 0b00000011]).is_err() {
        error!("Failed to write to AW9523 Port 1 Configuration register (0x03)");
        return Err(());
    }
    info!("AW9523: Port 1 Configuration set (0x03 <- 0x03)");

    // Step 3: Configure Port 0 Output (register 0x04) <- 0b00011000 (0x18)
    if i2c_device.write(AW9523_I2C_ADDRESS, &[0x04, 0b00011000]).is_err() {
        error!("Failed to write to AW9523 Port 0 Output register (0x04)");
        return Err(());
    }
    info!("AW9523: Port 0 Output set (0x04 <- 0x18)");

    // Step 4: Configure Port 1 Output (register 0x05) <- 0b00001100 (0x0C)
    if i2c_device.write(AW9523_I2C_ADDRESS, &[0x05, 0b00001100]).is_err() {
        error!("Failed to write to AW9523 Port 1 Output register (0x05)");
        return Err(());
    }
    info!("AW9523: Port 1 Output set (0x05 <- 0x0C)");

    // Step 5: Configure register 0x11 <- 0b00010000 (0x10)
    if i2c_device.write(AW9523_I2C_ADDRESS, &[0x11, 0b00010000]).is_err() {
        error!("Failed to write to AW9523 register (0x11)");
        return Err(());
    }
    info!("AW9523: Register 0x11 configured (0x11 <- 0x10)");

    // Step 6: Configure register 0x13 <- 0b11111111 (0xFF)
    if i2c_device.write(AW9523_I2C_ADDRESS, &[0x13, 0b11111111]).is_err() {
        error!("Failed to write to AW9523 register (0x13)");
        return Err(());
    }
    info!("AW9523: Register 0x13 configured (0x13 <- 0xFF)");

    info!("AW9523 GPIO expander initialized successfully with M5Stack CoreS3 sequence");
    Ok(())
}

impl EspBackend {
    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        // Take and configure peripherals.
        let peripherals = self.peripherals.borrow_mut().take().expect("Peripherals already taken");
        let mut delay = Delay::new();

        // --- Initialize I2C bus for all I2C devices (AXP2101, AW9523, touch controller) ---
        let power_i2c = I2c::new(
            peripherals.I2C0,
            esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
        )
        .unwrap()
        .with_sda(peripherals.GPIO12) // AXP2101 SDA
        .with_scl(peripherals.GPIO11); // AXP2101 SCL

        // --- Use StaticCell to create a shared I2C bus for all I2C devices ---
        static I2C_BUS: StaticCell<RefCell<I2c<'static, esp_hal::Blocking>>> = StaticCell::new();
        let i2c_bus = I2C_BUS.init(RefCell::new(power_i2c));

        // --- Begin AXP2101 Power Management Initialization ---
        // Initialize power management using shared I2C bus - critical for M5Stack CoreS3
        match init_axp2101_power(RefCellDevice::new(i2c_bus)) {
            Ok(_) => {
                info!("Power management initialized successfully");
            }
            Err(_) => {
                error!("Failed to initialize AXP2101 power management");
                // Return error since power management is critical
                return Err(slint::PlatformError::Other("AXP2101 initialization failed".into()));
            }
        };

        // Small delay to let power rails stabilize
        delay.delay_ms(100);

        // --- Begin AW9523 GPIO Expander Initialization ---
        // Initialize AW9523 GPIO expander using M5Stack CoreS3 specific sequence
        match init_aw9523_gpio_expander(RefCellDevice::new(i2c_bus)) {
            Ok(_) => {
                info!("AW9523 GPIO expander initialized successfully");
            }
            Err(_) => {
                error!("Failed to initialize AW9523 GPIO expander");
                // Return error since GPIO expander is needed for touch
                return Err(slint::PlatformError::Other("AW9523 initialization failed".into()));
            }
        };
        // --- End AW9523 Initialization ---

        // --- Begin SPI and Display Initialization ---
        let spi = Spi::<esp_hal::Blocking>::new(
            peripherals.SPI2,
            SpiConfig::default().with_frequency(Rate::from_mhz(40)).with_mode(SpiMode::_0),
        )
        .unwrap()
        .with_sck(peripherals.GPIO36) // SPI Clock
        .with_mosi(peripherals.GPIO37); // SPI MOSI

        // Display control pins
        let dc = Output::new(peripherals.GPIO35, Level::Low, OutputConfig::default()); // D/C pin
        let cs = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default()); // CS pin
        let reset = Output::new(peripherals.GPIO15, Level::High, OutputConfig::default()); // Reset pin

        // Wrap SPI into a bus.
        let spi_delay = Delay::new();
        let spi_device = ExclusiveDevice::new(spi, cs, spi_delay).unwrap();

        // Create buffer for display interface
        let mut buffer = [0u8; 512];
        let di = mipidsi::interface::SpiInterface::new(spi_device, dc, &mut buffer);

        // Add small delay before display initialization
        delay.delay_ms(10);

        // Initialize the display with settings
        let mut display = mipidsi::Builder::new(mipidsi::models::ILI9342CRgb565, di)
            .reset_pin(reset)
            .display_size(320, 240)
            .color_order(ColorOrder::Bgr)
            .invert_colors(ColorInversion::Inverted)
            .init(&mut delay)
            .unwrap();

        // Clear display to test it's working
        use embedded_graphics::pixelcolor::Rgb565;
        display
            .clear(Rgb565::BLUE)
            .map_err(|_| slint::PlatformError::Other("Display clear failed".into()))?;
        info!("Display initialized and cleared to blue");

        // Set up the backlight pin (controlled via AXP2101, but we can use GPIO for basic control)
        let mut backlight = Output::new(peripherals.GPIO16, Level::Low, OutputConfig::default());
        backlight.set_high(); // Enable backlight

        // Update the Slint window size from the display (320x240 for M5Stack CoreS3)
        let size = display.size();
        let size = slint::PhysicalSize::new(size.width, size.height);
        self.window.borrow().as_ref().unwrap().set_size(size);

        // --- End Display Initialization ---

        // --- Begin Touch Initialization ---
        info!("Initializing FT6336U touch controller...");

        // Create touch reset driver using shared I2C bus
        let touch_reset = TouchResetDriverAW9523::new(RefCellDevice::new(i2c_bus));

        // Initialize FT6336U touch driver using shared I2C bus
        let mut touch_driver = Ft3x68Driver::new(
            RefCellDevice::new(i2c_bus),
            FT6336U_DEVICE_ADDRESS,
            touch_reset,
            delay,
        );

        match touch_driver.initialize() {
            Ok(_) => info!("FT6336U touch controller initialized successfully"),
            Err(e) => {
                error!("Touch initialization failed: {:?}", e);
                // Continue without touch
            }
        }
        // --- End Touch Initialization ---

        // Prepare a draw buffer for the Slint software renderer
        let mut buffer_provider = DrawBuffer {
            display,
            buffer: &mut [slint::platform::software_renderer::Rgb565Pixel(0); 320],
        };

        // Variable to track the last touch position
        let mut last_touch = None;

        // Main event loop
        loop {
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                // Poll touch input using FT3x68Driver
                match touch_driver.touch1() {
                    Ok(touch_state) => {
                        match touch_state {
                            ft3x68_rs::TouchState::Pressed(touch_point) => {
                                info!("Touch detected: x={}, y={}", touch_point.x, touch_point.y);

                                // Convert touch coordinates to logical position
                                let pos = PhysicalPosition::new(
                                    touch_point.x as i32,
                                    touch_point.y as i32,
                                )
                                .to_logical(window.scale_factor());

                                if let Some(prev_pos) = last_touch.replace(pos) {
                                    // If position changed, send a PointerMoved event
                                    if prev_pos != pos {
                                        let _ =
                                            window.try_dispatch_event(WindowEvent::PointerMoved {
                                                position: pos,
                                            });
                                    }
                                } else {
                                    // No previous touch, send a PointerPressed event
                                    let _ =
                                        window.try_dispatch_event(WindowEvent::PointerPressed {
                                            position: pos,
                                            button: PointerEventButton::Left,
                                        });
                                }
                            }
                            ft3x68_rs::TouchState::Released => {
                                // Touch was released, send PointerReleased if we had a previous touch
                                if let Some(pos) = last_touch.take() {
                                    let _ =
                                        window.try_dispatch_event(WindowEvent::PointerReleased {
                                            position: pos,
                                            button: PointerEventButton::Left,
                                        });
                                    let _ = window.try_dispatch_event(WindowEvent::PointerExited);
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Touch error - ignore and continue
                    }
                }

                // Render the window if needed
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
    for &mut DrawBuffer<'_, mipidsi::Display<DI, mipidsi::models::ILI9342CRgb565, RST>>
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

        // Update the display with the rendered line
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
