// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
//! Board support for ESP32-S3-LCD-EV-Board with display and touch controller support.

extern crate alloc;

// --- Slint platform integration imports ---
use slint::platform::software_renderer::Rgb565Pixel;
// --- FT5x06 Touch Controller ---
struct Ft5x06<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C> Ft5x06<I2C>
where
    I2C: embedded_hal::i2c::I2c,
{
    pub fn new(i2c: I2C, address: u8) -> Self {
        Self { i2c, address }
    }

    /// Reads the first touch point. Returns Some((x, y)) if touched, None otherwise.
    pub fn get_touch(&mut self) -> Result<Option<(u16, u16)>, I2C::Error> {
        // 1) read touch count from register 0x02
        let mut buf = [0u8; 1];
        self.i2c.write_read(self.address, &[0x02], &mut buf)?;
        let count = buf[0] & 0x0F;
        if count == 0 {
            return Ok(None);
        }

        // 2) read first touch coordinates from regs 0x03..0x06
        let mut data = [0u8; 4];
        self.i2c.write_read(self.address, &[0x03], &mut data)?;
        let x = (((data[0] & 0x0F) as u16) << 8) | data[1] as u16;
        let y = (((data[2] & 0x0F) as u16) << 8) | data[3] as u16;

        Ok(Some((x, y)))
    }
}

use alloc::boxed::Box;
use esp_hal::dma::{DmaDescriptor, DmaTxBuf, CHUNK_SIZE};
use esp_hal::i2c;
use esp_hal::peripherals::Peripherals;
use slint::LogicalPosition;
use slint::PhysicalPosition;
use slint::PhysicalSize;

use alloc::rc::Rc;
use core::cell::RefCell;
use esp_hal::clock::CpuClock::_240MHz;
use esp_hal::delay::Delay;
use esp_hal::i2c::master::{Error, I2c};
use esp_hal::lcd_cam::{
    lcd::{
        dpi::{Config as DpiConfig, Dpi, Format, FrameTiming},
        ClockMode, Phase, Polarity,
    },
    LcdCam,
};
use esp_hal::time::Instant;
use esp_hal::{
    gpio::{Level, Output, OutputConfig},
    time::Rate,
    Blocking, Config as HalConfig,
};
use esp_println::logger::init_logger_from_env;
use i_slint_core::input::PointerEventButton;
use i_slint_core::platform::WindowEvent;
use log::{error, info};

// === Display constants ===
const LCD_H_RES: u16 = 480;
const LCD_V_RES: u16 = 480;
const FRAME_BYTES: usize = (LCD_H_RES as usize * LCD_V_RES as usize) * 2;
const NUM_DMA_DESC: usize = (FRAME_BYTES + CHUNK_SIZE - 1) / CHUNK_SIZE;

// Place DMA descriptors in DMA-capable RAM
#[link_section = ".dma"]
static mut TX_DESCRIPTORS: [DmaDescriptor; NUM_DMA_DESC] = [DmaDescriptor::EMPTY; NUM_DMA_DESC];

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    error!("Panic: {}", _info);
    loop {}
}

struct EspBackend {
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow>>>,
    peripherals: RefCell<Option<Peripherals>>,
}

impl Default for EspBackend {
    fn default() -> Self {
        EspBackend { window: RefCell::new(None), peripherals: RefCell::new(None) }
    }
}

/// Initialize the heap and set the Slint platform.
pub fn init() {
    // Initialize peripherals first.
    let peripherals = esp_hal::init(HalConfig::default().with_cpu_clock(_240MHz));
    init_logger_from_env();
    info!("Peripherals initialized");

    // Initialize the PSRAM allocator.
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);

    // Create and install the Slint backend that owns the peripherals.
    slint::platform::set_platform(Box::new(EspBackend {
        window: RefCell::new(None),
        peripherals: RefCell::new(Some(peripherals)),
    }))
    .expect("Slint platform already initialized");
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
        // Reinitialize peripherals, PSRAM, and logger
        let peripherals = self.peripherals.borrow_mut().take().expect("Peripherals already taken");

        // Setup I2C for the TCA9554 IO expander
        let i2c = I2c::new(
            peripherals.I2C0,
            i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
        )
        .unwrap()
        .with_sda(peripherals.GPIO47)
        .with_scl(peripherals.GPIO48);

        // Initialize the IO expander for controlling the display
        let mut expander = Tca9554::new(i2c);
        expander.write_output_reg(0b1111_0011).unwrap();
        expander.write_direction_reg(0b1111_0001).unwrap();

        let delay = Delay::new();
        info!("Initializing display...");

        // Set up the write_byte function for sending commands to the display
        let mut write_byte = |b: u8, is_cmd: bool| {
            const SCS_BIT: u8 = 0b0000_0010;
            const SCL_BIT: u8 = 0b0000_0100;
            const SDA_BIT: u8 = 0b0000_1000;

            let mut output = 0b1111_0001 & !SCS_BIT;
            expander.write_output_reg(output).unwrap();

            for bit in core::iter::once(!is_cmd).chain((0..8).map(|i| (b >> i) & 0b1 != 0).rev()) {
                let prev = output;
                if bit {
                    output |= SDA_BIT;
                } else {
                    output &= !SDA_BIT;
                }
                if prev != output {
                    expander.write_output_reg(output).unwrap();
                }

                output &= !SCL_BIT;
                expander.write_output_reg(output).unwrap();

                output |= SCL_BIT;
                expander.write_output_reg(output).unwrap();
            }

            output &= !SCL_BIT;
            expander.write_output_reg(output).unwrap();

            output &= !SDA_BIT;
            expander.write_output_reg(output).unwrap();

            output |= SCS_BIT;
            expander.write_output_reg(output).unwrap();
        };

        // VSYNC must be high during initialization
        let mut vsync_pin = peripherals.GPIO3;
        let vsync_guard = Output::new(vsync_pin.reborrow(), Level::High, OutputConfig::default());

        // Initialize the display by sending the initialization commands
        for &init in INIT_CMDS.iter() {
            match init {
                InitCmd::Cmd(cmd, args) => {
                    write_byte(cmd, true);
                    for &arg in args {
                        write_byte(arg, false);
                    }
                }
                InitCmd::Delay(ms) => {
                    delay.delay_millis(ms as _);
                }
            }
        }
        drop(vsync_guard);

        // Set up DMA channel for LCD
        let tx_channel = peripherals.DMA_CH2;
        let lcd_cam = LcdCam::new(peripherals.LCD_CAM);

        // Configure the RGB display
        let config = DpiConfig::default()
            .with_clock_mode(ClockMode { polarity: Polarity::IdleLow, phase: Phase::ShiftLow })
            .with_frequency(Rate::from_mhz(10))
            .with_format(Format { enable_2byte_mode: true, ..Default::default() })
            .with_timing(FrameTiming {
                horizontal_active_width: LCD_H_RES as usize,
                vertical_active_height: LCD_V_RES as usize,
                horizontal_total_width: 600,
                horizontal_blank_front_porch: 80,
                vertical_total_height: 600,
                vertical_blank_front_porch: 80,
                hsync_width: 10,
                vsync_width: 4,
                hsync_position: 10,
            })
            .with_vsync_idle_level(Level::High)
            .with_hsync_idle_level(Level::High)
            .with_de_idle_level(Level::Low)
            .with_disable_black_region(false);

        let mut dpi = Dpi::new(lcd_cam.lcd, tx_channel, config)
            .unwrap()
            .with_vsync(vsync_pin.reborrow())
            .with_hsync(peripherals.GPIO46)
            .with_de(peripherals.GPIO17)
            .with_pclk(peripherals.GPIO9)
            .with_data0(peripherals.GPIO10)
            .with_data1(peripherals.GPIO11)
            .with_data2(peripherals.GPIO12)
            .with_data3(peripherals.GPIO13)
            .with_data4(peripherals.GPIO14)
            .with_data5(peripherals.GPIO21)
            .with_data6(peripherals.GPIO8)
            .with_data7(peripherals.GPIO18)
            .with_data8(peripherals.GPIO45)
            .with_data9(peripherals.GPIO38)
            .with_data10(peripherals.GPIO39)
            .with_data11(peripherals.GPIO40)
            .with_data12(peripherals.GPIO41)
            .with_data13(peripherals.GPIO42)
            .with_data14(peripherals.GPIO2)
            .with_data15(peripherals.GPIO1);

        info!("Display initialized, entering main loop...");

        const FRAME_PIXELS: usize = (LCD_H_RES as usize) * (LCD_V_RES as usize);
        const FRAME_BYTES: usize = FRAME_PIXELS * 2;

        // Allocate a PSRAM-backed DMA buffer for the frame
        let buf_box: Box<[u8; FRAME_BYTES]> = Box::new([0; FRAME_BYTES]);
        let psram_buf: &'static mut [u8] = Box::leak(buf_box);
        let mut dma_tx: DmaTxBuf = unsafe {
            let descriptors = &mut *core::ptr::addr_of_mut!(TX_DESCRIPTORS);
            DmaTxBuf::new(descriptors, psram_buf).unwrap()
        };
        let mut pixel_box: Box<[Rgb565Pixel; FRAME_PIXELS]> =
            Box::new([Rgb565Pixel(0); FRAME_PIXELS]);
        let pixel_buf: &mut [Rgb565Pixel] = &mut *pixel_box;
        // Initialize pixel buffer and DMA buffer
        // The pixel buffer will be filled by Slint's renderer in the main loop
        let dst = dma_tx.as_mut_slice();
        for (i, px) in pixel_buf.iter().enumerate() {
            let [lo, hi] = px.0.to_le_bytes();
            dst[2 * i] = lo;
            dst[2 * i + 1] = hi;
        }
        // Initial flush of the screen buffer
        match dpi.send(false, dma_tx) {
            Ok(xfer) => {
                let (_res, dpi2, tx2) = xfer.wait();
                dpi = dpi2;
                dma_tx = tx2;
            }
            Err((e, dpi2, tx2)) => {
                error!("Initial DMA send error: {:?}", e);
                dpi = dpi2;
                dma_tx = tx2;
            }
        }

        // Tell Slint the window dimensions match the DPI display resolution
        let size = PhysicalSize::new(LCD_H_RES.into(), LCD_V_RES.into());
        self.window.borrow().as_ref().expect("Window adapter not created").set_size(size);

        // Initialize FT5x06 touch controller on I2C1 (example pins)
        // Reclaim the I2C bus from the expander for FT5x06
        let i2c_bus = expander.into_i2c();
        let mut touch = Ft5x06::new(i2c_bus, 0x38);
        let mut last_touch: Option<LogicalPosition> = None;

        loop {
            // 1) Let Slint update its timers and animations
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                window.request_redraw();
            }

            if let Some(window) = self.window.borrow().clone() {
                // Poll FT5x06 touch each frame since INT line is NC
                if let Ok(Some((x, y))) = touch.get_touch() {
                    let pos =
                        PhysicalPosition::new(x as i32, y as i32).to_logical(window.scale_factor());
                    if let Some(prev) = last_touch.replace(pos) {
                        if prev != pos {
                            window
                                .try_dispatch_event(WindowEvent::PointerMoved { position: pos })?;
                        }
                    } else {
                        window.try_dispatch_event(WindowEvent::PointerPressed {
                            position: pos,
                            button: PointerEventButton::Left,
                        })?;
                    }
                } else if let Some(pos) = last_touch.take() {
                    window.try_dispatch_event(WindowEvent::PointerReleased {
                        position: pos,
                        button: PointerEventButton::Left,
                    })?;
                    window.try_dispatch_event(WindowEvent::PointerExited)?;
                }

                // 2) Render the UI into Slint's software renderer buffer
                window.draw_if_needed(|renderer| {
                    let _dirty = renderer.render(pixel_buf, LCD_H_RES as usize);
                });

                // 3) Pack pixels into DMA buffer
                {
                    let dst = dma_tx.as_mut_slice();
                    for (i, px) in pixel_buf.iter().enumerate() {
                        let [lo, hi] = px.0.to_le_bytes();
                        dst[2 * i] = lo;
                        dst[2 * i + 1] = hi;
                    }
                }

                // 3) One-shot DMA transfer of the full frame
                match dpi.send(false, dma_tx) {
                    Ok(xfer) => {
                        let (res, dpi2, tx2) = xfer.wait();
                        dpi = dpi2;
                        dma_tx = tx2;
                        if let Err(e) = res {
                            error!("DMA error: {:?}", e);
                        }
                    }
                    Err((e, dpi2, tx2)) => {
                        error!("DMA send error: {:?}", e);
                        dpi = dpi2;
                        dma_tx = tx2;
                    }
                }

                // 4) If there are active animations, continue immediately
                if window.has_active_animations() {
                    continue;
                }
            }
        }
    }
}

// --- I2C expander (TCA9554) ---
struct Tca9554 {
    i2c: I2c<'static, esp_hal::Blocking>,
    address: u8,
}

impl Tca9554 {
    pub fn new(i2c: I2c<'static, esp_hal::Blocking>) -> Self {
        Self { i2c, address: 0x20 }
    }
    pub fn write_direction_reg(&mut self, value: u8) -> Result<(), Error> {
        self.i2c.write(self.address, &[0x03, value])
    }
    pub fn write_output_reg(&mut self, value: u8) -> Result<(), Error> {
        self.i2c.write(self.address, &[0x01, value])
    }

    pub fn into_i2c(self) -> I2c<'static, Blocking> {
        self.i2c
    }
}

// Display initialization commands for the ESP32-S3-LCD-EV-Board
#[derive(Copy, Clone, Debug)]
enum InitCmd {
    Cmd(u8, &'static [u8]),
    Delay(u8),
}

const INIT_CMDS: &[InitCmd] = &[
    InitCmd::Cmd(0xf0, &[0x55, 0xaa, 0x52, 0x08, 0x00]),
    InitCmd::Cmd(0xf6, &[0x5a, 0x87]),
    InitCmd::Cmd(0xc1, &[0x3f]),
    InitCmd::Cmd(0xc2, &[0x0e]),
    InitCmd::Cmd(0xc6, &[0xf8]),
    InitCmd::Cmd(0xc9, &[0x10]),
    InitCmd::Cmd(0xcd, &[0x25]),
    InitCmd::Cmd(0xf8, &[0x8a]),
    InitCmd::Cmd(0xac, &[0x45]),
    InitCmd::Cmd(0xa0, &[0xdd]),
    InitCmd::Cmd(0xa7, &[0x47]),
    InitCmd::Cmd(0xfa, &[0x00, 0x00, 0x00, 0x04]),
    InitCmd::Cmd(0x86, &[0x99, 0xa3, 0xa3, 0x51]),
    InitCmd::Cmd(0xa3, &[0xee]),
    InitCmd::Cmd(0xfd, &[0x3c, 0x3]),
    InitCmd::Cmd(0x71, &[0x48]),
    InitCmd::Cmd(0x72, &[0x48]),
    InitCmd::Cmd(0x73, &[0x00, 0x44]),
    InitCmd::Cmd(0x97, &[0xee]),
    InitCmd::Cmd(0x83, &[0x93]),
    InitCmd::Cmd(0x9a, &[0x72]),
    InitCmd::Cmd(0x9b, &[0x5a]),
    InitCmd::Cmd(0x82, &[0x2c, 0x2c]),
    InitCmd::Cmd(0xB1, &[0x10]),
    InitCmd::Cmd(
        0x6d,
        &[
            0x00, 0x1f, 0x19, 0x1a, 0x10, 0x0e, 0x0c, 0x0a, 0x02, 0x07, 0x1e, 0x1e, 0x1e, 0x1e,
            0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x08, 0x01, 0x09, 0x0b, 0x0d, 0x0f,
            0x1a, 0x19, 0x1f, 0x00,
        ],
    ),
    InitCmd::Cmd(
        0x64,
        &[
            0x38, 0x05, 0x01, 0xdb, 0x03, 0x03, 0x38, 0x04, 0x01, 0xdc, 0x03, 0x03, 0x7a, 0x7a,
            0x7a, 0x7a,
        ],
    ),
    InitCmd::Cmd(
        0x65,
        &[
            0x38, 0x03, 0x01, 0xdd, 0x03, 0x03, 0x38, 0x02, 0x01, 0xde, 0x03, 0x03, 0x7a, 0x7a,
            0x7a, 0x7a,
        ],
    ),
    InitCmd::Cmd(
        0x66,
        &[
            0x38, 0x01, 0x01, 0xdf, 0x03, 0x03, 0x38, 0x00, 0x01, 0xe0, 0x03, 0x03, 0x7a, 0x7a,
            0x7a, 0x7a,
        ],
    ),
    InitCmd::Cmd(
        0x67,
        &[
            0x30, 0x01, 0x01, 0xe1, 0x03, 0x03, 0x30, 0x02, 0x01, 0xe2, 0x03, 0x03, 0x7a, 0x7a,
            0x7a, 0x7a,
        ],
    ),
    InitCmd::Cmd(
        0x68,
        &[0x00, 0x08, 0x15, 0x08, 0x15, 0x7a, 0x7a, 0x08, 0x15, 0x08, 0x15, 0x7a, 0x7a],
    ),
    InitCmd::Cmd(0x60, &[0x38, 0x08, 0x7a, 0x7a, 0x38, 0x09, 0x7a, 0x7a]),
    InitCmd::Cmd(0x63, &[0x31, 0xe4, 0x7a, 0x7a, 0x31, 0xe5, 0x7a, 0x7a]),
    InitCmd::Cmd(0x69, &[0x04, 0x22, 0x14, 0x22, 0x14, 0x22, 0x08]),
    InitCmd::Cmd(0x6b, &[0x07]),
    InitCmd::Cmd(0x7a, &[0x08, 0x13]),
    InitCmd::Cmd(0x7b, &[0x08, 0x13]),
    InitCmd::Cmd(
        0xd1,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(
        0xd2,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(
        0xd3,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(
        0xd4,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(
        0xd5,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(
        0xd6,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(0x36, &[0x00]),
    InitCmd::Cmd(0x2A, &[0x00, 0x00, 0x01, 0xDF]), // 0 to 479 (0x1DF)
    // Set full row address range
    InitCmd::Cmd(0x2B, &[0x00, 0x00, 0x01, 0xDF]), // 0 to 479 (0x1DF)
    InitCmd::Cmd(0x3A, &[0x66]),
    InitCmd::Cmd(0x11, &[]),
    InitCmd::Delay(120),
    InitCmd::Cmd(0x29, &[]),
    InitCmd::Delay(20),
];
