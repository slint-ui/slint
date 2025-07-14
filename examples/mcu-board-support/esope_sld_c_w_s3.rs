// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
//! Board support for ESoPe-SLD-C-W-S3 board with display and touch controller support.

#![no_std]

extern crate alloc;

// Import embedded_graphics_core types
use embedded_graphics_core::{pixelcolor::Rgb565, prelude::*};
use embedded_graphics_framebuf::backends::FrameBufferBackend;
use embedded_graphics_framebuf::FrameBuf;

// --- Slint platform integration imports ---
use slint::platform::software_renderer::Rgb565Pixel;
use slint::PhysicalSize;

use alloc::alloc::{alloc, handle_alloc_error};
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::alloc::Layout;
use core::cell::RefCell;

use eeprom24x::{Eeprom24x, SlaveAddr};
use esp_hal::clock::CpuClock;
use esp_hal::dma::{DmaDescriptor, DmaTxBuf, ExternalBurstConfig, CHUNK_SIZE};
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::I2c;
use esp_hal::lcd_cam::{
    lcd::{
        dpi::{Config as DpiConfig, Dpi, Format, FrameTiming},
        ClockMode, Phase, Polarity,
    },
    LcdCam,
};
use esp_hal::peripherals::Peripherals;
use esp_hal::system::{CpuControl, Stack};
use esp_hal::time::{Instant, Rate};
use esp_hal::timer::{timg::TimerGroup, AnyTimer};
use esp_hal::Config as HalConfig;
use esp_println::logger::init_logger_from_env;
use log::{error, info};

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Ticker;
use esp_hal_embassy::Executor;
use static_cell::StaticCell;

// Constants matching Conway's implementation
const LCD_H_RES_USIZE: usize = 320;
const LCD_V_RES_USIZE: usize = 240;
const LCD_BUFFER_SIZE: usize = LCD_H_RES_USIZE * LCD_V_RES_USIZE;

// Embassy multicore: allocate app core stack
static mut APP_CORE_STACK: Stack<8192> = Stack::new();

static PSRAM_READY: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static mut PSRAM_BUF_PTR: *mut u8 = core::ptr::null_mut();
static mut PSRAM_BUF_LEN: usize = 0;

// === Display constants ===
const LCD_H_RES: u16 = 320;
const LCD_V_RES: u16 = 240;

// Full-screen DMA constants
const MAX_FRAME_BYTES: usize = 320 * 240 * 2;
const MAX_NUM_DMA_DESC: usize = (MAX_FRAME_BYTES + CHUNK_SIZE - 1) / CHUNK_SIZE;

#[unsafe(link_section = ".dma")]
static mut TX_DESCRIPTORS: [DmaDescriptor; MAX_NUM_DMA_DESC] =
    [DmaDescriptor::EMPTY; MAX_NUM_DMA_DESC];

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
    let config = HalConfig::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
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

/// FrameBufferBackend wrapper for a PSRAM-backed [Rgb565; N] slice.
pub struct PSRAMFrameBuffer<'a> {
    buf: &'a mut [Rgb565; LCD_BUFFER_SIZE],
}

impl<'a> PSRAMFrameBuffer<'a> {
    pub fn new(buf: &'a mut [Rgb565; LCD_BUFFER_SIZE]) -> Self {
        Self { buf }
    }
}

impl<'a> FrameBufferBackend for PSRAMFrameBuffer<'a> {
    type Color = Rgb565;
    fn set(&mut self, index: usize, color: Self::Color) {
        self.buf[index] = color;
    }
    fn get(&self, index: usize) -> Self::Color {
        self.buf[index]
    }
    fn nr_elements(&self) -> usize {
        LCD_BUFFER_SIZE
    }
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
        let peripherals = self.peripherals.borrow_mut().take().expect("Peripherals already taken");

        // Read and set up the display configuration from EEPROM
        let i2c_bus = I2c::new(peripherals.I2C0, esp_hal::i2c::master::Config::default())
            .unwrap()
            .with_sda(peripherals.GPIO1)
            .with_scl(peripherals.GPIO41);
        let mut eeid = [0u8; 0x1c];
        let mut eeprom = Eeprom24x::new_24x01(i2c_bus, SlaveAddr::default());
        eeprom.read_data(0x00, &mut eeid).unwrap();
        let display_width = u16::from_be_bytes([eeid[8], eeid[9]]);
        let display_height = u16::from_be_bytes([eeid[10], eeid[11]]);
        info!("Display size from EEPROM: {}x{}", display_width, display_height);

        // Enable panel / backlight
        let mut panel_enable = Output::new(peripherals.GPIO42, Level::Low, OutputConfig::default());
        panel_enable.set_high();

        let mut backlight = Output::new(peripherals.GPIO39, Level::Low, OutputConfig::default());
        backlight.set_high();

        let mut _touch_reset = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

        info!("Display initialized, entering main loop...");

        // Allocate framebuffer in PSRAM and reuse as DMA buffer with proper 64-byte alignment
        const FRAME_BYTES: usize = LCD_BUFFER_SIZE * 2;

        // Use manual allocation with alignment like Conway's implementation
        let layout = Layout::from_size_align(FRAME_BYTES, 64).unwrap();
        let fb_ptr = unsafe {
            let ptr = alloc(layout);
            if ptr.is_null() {
                error!("Failed to allocate aligned PSRAM buffer");
                handle_alloc_error(layout);
            }
            ptr
        };

        // Initialize the buffer with zeros
        unsafe {
            core::ptr::write_bytes(fb_ptr, 0, FRAME_BYTES);
        }

        let psram_buf: &'static mut [u8] =
            unsafe { core::slice::from_raw_parts_mut(fb_ptr, FRAME_BYTES) };

        // Verify PSRAM buffer allocation and alignment
        let buf_ptr = psram_buf.as_ptr() as usize;
        info!("PSRAM buffer allocated at address: 0x{:08X}", buf_ptr);
        info!("PSRAM buffer length: {}", psram_buf.len());
        info!("PSRAM buffer alignment modulo 64: {}", buf_ptr % 64);

        // Assert that we have proper 64-byte alignment for DMA
        assert_eq!(buf_ptr % 64, 0, "PSRAM buffer must be 64-byte aligned for DMA");
        info!("PSRAM buffer is properly 64-byte aligned for DMA");

        // Publish PSRAM buffer pointer and len for app core
        unsafe {
            PSRAM_BUF_PTR = psram_buf.as_mut_ptr();
            PSRAM_BUF_LEN = FRAME_BYTES;
        }

        // Configure DMA buffer with proper burst configuration
        let mut dma_tx: DmaTxBuf = unsafe {
            let descriptors = &mut *core::ptr::addr_of_mut!(TX_DESCRIPTORS);
            DmaTxBuf::new_with_config(descriptors, psram_buf, ExternalBurstConfig::Size64).unwrap()
        };

        // Allocate pixel buffer for Slint rendering
        const FRAME_PIXELS: usize = LCD_BUFFER_SIZE;
        let mut pixel_box: Box<[Rgb565Pixel; FRAME_PIXELS]> =
            Box::new([Rgb565Pixel(0); FRAME_PIXELS]);
        let pixel_buf: &mut [Rgb565Pixel] = &mut *pixel_box;

        // Initialize LCD DPI interface
        let lcd_cam = LcdCam::new(peripherals.LCD_CAM);

        // Read configuration from EEPROM
        let pclk_hz = ((eeid[12] as u32) * 1_000_000 + (eeid[13] as u32) * 100_000).min(13_600_000);
        let flags = eeid[25];
        let hsync_idle_low = (flags & 0x01) != 0;
        let vsync_idle_low = (flags & 0x02) != 0;
        let de_idle_high = (flags & 0x04) != 0;
        let pclk_active_neg = (flags & 0x20) != 0;

        // Log display configuration to match Conway's working values
        info!("Display configuration:");
        info!("  Resolution: {}x{}", display_width, display_height);
        info!("  PCLK: {} Hz", pclk_hz);
        info!("  Flags: 0x{:02X}", flags);
        info!("  HSYNC idle low: {}", hsync_idle_low);
        info!("  VSYNC idle low: {}", vsync_idle_low);
        info!("  DE idle high: {}", de_idle_high);
        info!("  PCLK active neg: {}", pclk_active_neg);

        let dpi_config = DpiConfig::default()
            .with_clock_mode(ClockMode {
                polarity: if pclk_active_neg { Polarity::IdleHigh } else { Polarity::IdleLow },
                phase: if pclk_active_neg { Phase::ShiftHigh } else { Phase::ShiftLow },
            })
            .with_frequency(Rate::from_hz(pclk_hz))
            .with_format(Format { enable_2byte_mode: true, ..Default::default() })
            // Use exact timing values that work with Conway's implementation
            .with_timing(FrameTiming {
                horizontal_active_width: 320,
                horizontal_total_width: 320 + 4 + 43 + 79 + 8, // =446 (Conway's working value)
                horizontal_blank_front_porch: 79 + 8,          // was 47, add 32px
                vertical_active_height: 240,
                vertical_total_height: 240 + 4 + 12 + 16, // increased blank front porch to 16
                vertical_blank_front_porch: 16,
                hsync_width: 4,
                vsync_width: 4,
                hsync_position: 43 + 4, // (= back_porch + pulse = 47) Conway's working value
            })
            .with_vsync_idle_level(if vsync_idle_low { Level::Low } else { Level::High })
            .with_hsync_idle_level(if hsync_idle_low { Level::Low } else { Level::High })
            .with_de_idle_level(if de_idle_high { Level::High } else { Level::Low })
            .with_disable_black_region(false);

        let mut dpi = Dpi::new(lcd_cam.lcd, peripherals.DMA_CH2, dpi_config)
            .unwrap()
            .with_vsync(peripherals.GPIO6)
            .with_hsync(peripherals.GPIO15)
            .with_de(peripherals.GPIO5)
            .with_pclk(peripherals.GPIO4)
            // Blue bus
            .with_data0(peripherals.GPIO9)
            .with_data1(peripherals.GPIO17)
            .with_data2(peripherals.GPIO46)
            .with_data3(peripherals.GPIO16)
            .with_data4(peripherals.GPIO7)
            // Green bus
            .with_data5(peripherals.GPIO8)
            .with_data6(peripherals.GPIO21)
            .with_data7(peripherals.GPIO3)
            .with_data8(peripherals.GPIO11)
            .with_data9(peripherals.GPIO18)
            .with_data10(peripherals.GPIO10)
            // Red bus
            .with_data11(peripherals.GPIO14)
            .with_data12(peripherals.GPIO20)
            .with_data13(peripherals.GPIO13)
            .with_data14(peripherals.GPIO19)
            .with_data15(peripherals.GPIO12);

        // Initialize pixel buffer with a test pattern to verify DMA is working
        // Fill with alternating red/blue checkerboard pattern
        for (i, px) in pixel_buf.iter_mut().enumerate() {
            let x = i % (LCD_H_RES as usize);
            let y = i / (LCD_H_RES as usize);
            let checker = ((x / 32) + (y / 32)) % 2;
            *px = if checker == 0 {
                Rgb565Pixel(0xF800) // Red
            } else {
                Rgb565Pixel(0x001F) // Blue
            };
        }

        // Pack initial test pattern into DMA buffer
        let dst = dma_tx.as_mut_slice();
        for (i, px) in pixel_buf.iter().enumerate() {
            let [lo, hi] = px.0.to_le_bytes();
            dst[2 * i] = lo;
            dst[2 * i + 1] = hi;
        }

        // Initial flush of the screen buffer
        info!("Sending initial test pattern to display...");
        match dpi.send(false, dma_tx) {
            Ok(xfer) => {
                let (res, dpi2, tx2) = xfer.wait();
                dpi = dpi2;
                dma_tx = tx2;
                if let Err(e) = res {
                    error!("Initial DMA error: {:?}", e);
                } else {
                    info!("Initial test pattern sent successfully");
                }
            }
            Err((e, dpi2, tx2)) => {
                error!("Initial DMA send error: {:?}", e);
                dpi = dpi2;
                dma_tx = tx2;
            }
        }

        // Tell Slint the window dimensions match the display resolution
        let size = PhysicalSize::new(LCD_H_RES.into(), LCD_V_RES.into());
        self.window.borrow().as_ref().expect("Window adapter not created").set_size(size);

        PSRAM_READY.signal(());
        let timg0 = TimerGroup::new(peripherals.TIMG0);
        let timer0: AnyTimer = timg0.timer0.into();

        // Spawn Conway update task on app core (core 1)
        let mut cpu_control = CpuControl::new(peripherals.CPU_CTRL);
        let _app_core = cpu_control.start_app_core(
            unsafe { &mut *core::ptr::addr_of_mut!(APP_CORE_STACK) },
            move || {
                // Initialize TimerGroup and timer1 for app core Embassy time driver
                let timg1 = TimerGroup::new(unsafe { esp_hal::peripherals::TIMG1::steal() });
                let timer1: AnyTimer = timg1.timer0.into();
                // Initialize Embassy time driver on app core
                esp_hal_embassy::init([timer0, timer1]);
                // SAFETY: PSRAM_BUF_PTR and PSRAM_BUF_LEN are published before
                let psram_ptr = unsafe { PSRAM_BUF_PTR };
                let psram_len = unsafe { PSRAM_BUF_LEN };
                // Wait until PSRAM is ready
                loop {
                    if PSRAM_READY.try_take().is_some() {
                        break;
                    }
                    // Simple spin wait
                    core::hint::spin_loop();
                }
                // Initialize and run Embassy executor on app core
                static EXECUTOR: StaticCell<Executor> = StaticCell::new();
                let executor = EXECUTOR.init(Executor::new());
                executor.run(|spawner| {
                    spawner.spawn(slint_task(psram_ptr, psram_len)).ok();
                });
            },
        );

        // Core 0: Only send DMA frames in a loop
        loop {
            // println!("Core {}: Pushing DMA data...", Cpu::current() as usize);
            let safe_chunk_size = 320 * 240 * 2;
            let frame_bytes = display_width * display_height * 2;
            let len = safe_chunk_size.min(frame_bytes as usize);
            dma_tx.set_length(len);
            match dpi.send(false, dma_tx) {
                Ok(xfer) => {
                    let (res, new_dpi, new_dma_tx) = xfer.wait();
                    dpi = new_dpi;
                    dma_tx = new_dma_tx;
                    if let Err(e) = res {
                        error!("DMA transfer error: {:?}", e);
                    }
                }
                Err((e, new_dpi, new_dma_tx)) => {
                    error!("DMA send error: {:?}", e);
                    dpi = new_dpi;
                    dma_tx = new_dma_tx;
                }
            }
        }
    }
}

#[embassy_executor::task]
async fn slint_task(psram_ptr: *mut u8, _psram_len: usize) {
    // Reconstruct the framebuffer
    let fb: &mut [Rgb565; LCD_BUFFER_SIZE] =
        unsafe { &mut *(psram_ptr as *mut [Rgb565; LCD_BUFFER_SIZE]) };

    let mut frame_buf =
        FrameBuf::new(PSRAMFrameBuffer::new(fb), LCD_H_RES_USIZE.into(), LCD_V_RES_USIZE.into());
    let mut ticker = Ticker::every(embassy_time::Duration::from_millis(100));

    loop {
        // Update Slint timers and animations
        slint::platform::update_timers_and_animations();

        // For now, just fill the framebuffer with a simple pattern
        // In a real implementation, this would render the Slint UI
        frame_buf.clear(Rgb565::BLUE).ok();

        ticker.next().await;
    }
}
