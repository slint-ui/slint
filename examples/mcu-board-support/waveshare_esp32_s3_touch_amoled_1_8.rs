// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use embedded_hal::delay::DelayNs;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::dma_buffers;
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::peripherals::Peripherals;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode;
use esp_hal::time::{Instant, Rate};
use esp_println::logger::init_logger_from_env;
use log::{error, info};
use sh8601_rs::{
    framebuffer_size, ColorMode, DisplaySize, ResetDriver, Sh8601Driver, Ws18AmoledDriver,
    DMA_CHUNK_SIZE,
};
use slint::Rgb8Pixel;

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

impl EspBackend {
    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        // Take and configure peripherals.
        let peripherals = self.peripherals.borrow_mut().take().expect("Peripherals already taken");
        let mut delay = Delay::new();

        // Display configuration for Waveshare ESP32-S3-Touch-AMOLED-1.8
        const DISPLAY_SIZE: DisplaySize = DisplaySize::new(368, 448);
        const FB_SIZE: usize = framebuffer_size(DISPLAY_SIZE, ColorMode::Rgb888);

        // --- Begin SPI and Display Initialization ---
        // DMA Buffers for SPI
        let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(DMA_CHUNK_SIZE);
        let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
        let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

        // SPI Configuration for Waveshare ESP32-S3 1.8inch AMOLED Touch Display
        let lcd_spi = Spi::new(
            peripherals.SPI2,
            SpiConfig::default().with_frequency(Rate::from_mhz(40_u32)).with_mode(Mode::_0),
        )
        .unwrap()
        .with_sio0(peripherals.GPIO4)
        .with_sio1(peripherals.GPIO5)
        .with_sio2(peripherals.GPIO6)
        .with_sio3(peripherals.GPIO7)
        .with_cs(peripherals.GPIO12)
        .with_sck(peripherals.GPIO11)
        .with_dma(peripherals.DMA_CH0)
        .with_buffers(dma_rx_buf, dma_tx_buf);

        // I2C Configuration for Waveshare ESP32-S3 1.8inch AMOLED Touch Display
        let i2c =
            I2c::new(peripherals.I2C0, I2cConfig::default().with_frequency(Rate::from_khz(400)))
                .unwrap()
                .with_sda(peripherals.GPIO15)
                .with_scl(peripherals.GPIO14);

        // Initialize I2C GPIO Reset Pin for the WaveShare 1.8" AMOLED display
        let reset = ResetDriver::new(i2c);

        // Initialize display driver for the Waveshare 1.8" AMOLED display
        let ws_driver = Ws18AmoledDriver::new(lcd_spi);

        // Instantiate and Initialize Display
        info!("Initializing SH8601 Display...");
        let mut display = Sh8601Driver::new_heap::<_, FB_SIZE>(
            ws_driver,
            reset,
            ColorMode::Rgb888,
            DISPLAY_SIZE,
            delay,
        )
        .map_err(|e| {
            error!("Error initializing display: {:?}", e);
            slint::PlatformError::Other("Display initialization failed".into())
        })?;

        info!("Display initialized successfully");

        // Update the Slint window size from the display
        let size = slint::PhysicalSize::new(DISPLAY_SIZE.width as u32, DISPLAY_SIZE.height as u32);
        self.window.borrow().as_ref().unwrap().set_size(size);

        // --- End Display Initialization ---

        // Allocate a full-screen buffer for rendering
        const FRAME_PIXELS: usize = (368 * 448) as usize;
        let mut pixel_buffer: Box<[Rgb8Pixel; FRAME_PIXELS]> =
            Box::new([Rgb8Pixel { r: 0, g: 0, b: 0 }; FRAME_PIXELS]);
        let pixel_buf: &mut [Rgb8Pixel] = &mut *pixel_buffer;

        // Variable to track the last touch position
        let _last_touch: Option<()> = None;

        info!("Entering main event loop...");

        // Main event loop
        loop {
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                // TODO: Add touch support for FT3168 when available
                // For now, the display will work without touch input

                // Render the window if needed
                window.draw_if_needed(|renderer| {
                    renderer.render(pixel_buf, DISPLAY_SIZE.width as usize);
                });

                // Draw the rendered pixels to the display using draw_iter for better performance
                use embedded_graphics::prelude::*;
                use embedded_graphics::Pixel;

                let pixels = pixel_buf
                    .chunks_exact(DISPLAY_SIZE.width as usize)
                    .enumerate()
                    .flat_map(|(y, row)| {
                        row.iter().enumerate().map(move |(x, pixel)| {
                            let point = embedded_graphics::geometry::Point::new(x as i32, y as i32);
                            let color = embedded_graphics::pixelcolor::Rgb888::new(
                                pixel.r, pixel.g, pixel.b,
                            );
                            Pixel(point, color)
                        })
                    });

                let _ = display.draw_iter(pixels);

                // Flush the display to show the rendered content
                let _ = display.flush();

                if window.has_active_animations() {
                    continue;
                }
            }

            // Small delay to prevent busy waiting
            delay.delay_ms(16); // ~60 FPS
        }
    }
}
