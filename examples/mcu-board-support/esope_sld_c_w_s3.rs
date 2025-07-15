// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
//! Board support for ESoPe-SLD-C-W-S3 board with display and touch controller support.

#![no_std]

extern crate alloc;

// Import embedded_graphics_core types
use embedded_graphics_core::pixelcolor::Rgb565;
use embedded_graphics_framebuf::backends::FrameBufferBackend;
// --- Slint platform integration imports ---
use slint::platform::software_renderer::Rgb565Pixel;
use slint::PhysicalSize;

use alloc::alloc::{alloc, handle_alloc_error};
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::alloc::Layout;
use core::cell::RefCell;

use eeprom24x::{Eeprom24x, SlaveAddr};
use embedded_hal_bus::i2c::RefCellDevice;
use esp_hal::clock::CpuClock;
use esp_hal::dma::ExternalBurstConfig;
use esp_hal::dma::{DmaDescriptor, DmaTxBuf, CHUNK_SIZE};
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::I2c;
use esp_hal::lcd_cam::{
    lcd::{
        dpi::{Config as DpiConfig, Dpi, Format, FrameTiming},
        ClockMode, Phase, Polarity,
    },
    LcdCam,
};

// Type alias for I2C device to simplify signatures
type I2cDevice = RefCellDevice<'static, esp_hal::i2c::master::I2c<'static, esp_hal::Blocking>>;
type TouchController = sitronix_touch::TouchIC<I2cDevice>;
use esp_hal::peripherals::Peripherals;
use esp_hal::system::{CpuControl, Stack};
use esp_hal::time::{Instant, Rate};
use esp_hal::timer::{timg::TimerGroup, AnyTimer};
use esp_hal::Config as HalConfig;
use esp_println::logger::init_logger_from_env;
use log::{debug, error, info};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Ticker, Timer};
use esp_hal_embassy::Executor;
use static_cell::StaticCell;

// Static storage for I2C bus
static I2C_BUS: StaticCell<RefCell<I2c<'static, esp_hal::Blocking>>> = StaticCell::new();

// Constants matching Conway's implementation
const LCD_H_RES_USIZE: usize = 320;
const LCD_V_RES_USIZE: usize = 240;
const LCD_BUFFER_SIZE: usize = LCD_H_RES_USIZE * LCD_V_RES_USIZE;

// Embassy multicore: allocate app core stack with reduced size to save memory
static mut APP_CORE_STACK: Stack<4096> = Stack::new();

static PSRAM_READY: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static DMA_READY: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static FRAME_READY: Signal<CriticalSectionRawMutex, ()> = Signal::new();
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
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!("PANIC: {}", info);
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
    info!("=== ESP32-S3 ESoPe Board Initialization Starting ===");
    info!("Peripherals initialized");

    // Log memory status before PSRAM init
    let heap_start = esp_alloc::HEAP.used();
    info!("Heap usage before PSRAM init: {} bytes", heap_start);

    // Initialize the PSRAM allocator.
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);
    info!("PSRAM allocator initialized");

    // Log memory status after PSRAM init
    let heap_after_psram = esp_alloc::HEAP.used();
    info!(
        "Heap usage after PSRAM init: {} bytes (delta: +{})",
        heap_after_psram,
        heap_after_psram.saturating_sub(heap_start)
    );

    // Create and install the Slint backend that owns the peripherals.
    info!("Creating Slint platform backend...");
    let heap_before_backend = esp_alloc::HEAP.used();

    slint::platform::set_platform(Box::new(EspBackend {
        window: RefCell::new(None),
        peripherals: RefCell::new(Some(peripherals)),
    }))
    .expect("Slint platform already initialized");

    let heap_after_backend = esp_alloc::HEAP.used();
    info!(
        "Slint backend created. Heap usage: {} bytes (delta: +{})",
        heap_after_backend,
        heap_after_backend.saturating_sub(heap_before_backend)
    );
    info!("=== Initialization Complete ===");
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
        info!("=== Starting Main Event Loop ===");
        let heap_at_start = esp_alloc::HEAP.used();
        info!("Heap usage at event loop start: {} bytes", heap_at_start);

        let peripherals = self.peripherals.borrow_mut().take().expect("Peripherals already taken");

        // Read and set up the display configuration from EEPROM
        let i2c = I2c::new(peripherals.I2C0, esp_hal::i2c::master::Config::default())
            .unwrap()
            .with_sda(peripherals.GPIO1)
            .with_scl(peripherals.GPIO41);
        let i2c_bus = I2C_BUS.init(RefCell::new(i2c));
        let mut eeid = [0u8; 0x1c];
        let mut eeprom = Eeprom24x::new_24x01(RefCellDevice::new(i2c_bus), SlaveAddr::default());
        eeprom.read_data(0x00, &mut eeid).unwrap();
        let display_width = u16::from_be_bytes([eeid[8], eeid[9]]);
        let display_height = u16::from_be_bytes([eeid[10], eeid[11]]);
        info!("Display size from EEPROM: {}x{}", display_width, display_height);

        // Initialize touch controller using shared I2C bus
        info!("Initializing touch controller...");
        let touch_device = RefCellDevice::new(i2c_bus);
        let mut touch_controller = sitronix_touch::TouchIC::new_default(touch_device);
        match touch_controller.init() {
            Ok(_) => info!("Touch controller initialized successfully"),
            Err(e) => {
                error!("Failed to initialize touch controller: {:?}", e);
                // Continue without touch support
            }
        }

        // Enable panel / backlight
        let mut panel_enable = Output::new(peripherals.GPIO42, Level::Low, OutputConfig::default());
        panel_enable.set_high();

        let mut backlight = Output::new(peripherals.GPIO39, Level::Low, OutputConfig::default());
        backlight.set_high();

        let mut _touch_reset = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

        info!("Display initialized, entering main loop...");

        // Allocate framebuffer in PSRAM with 64-byte alignment for DMA
        const FRAME_BYTES: usize = LCD_BUFFER_SIZE * 2;

        // Use aligned allocation for DMA requirements
        let layout = Layout::from_size_align(FRAME_BYTES, 64)
            .expect("Failed to create layout for framebuffer");
        let fb_ptr = unsafe { alloc(layout) };

        if fb_ptr.is_null() {
            handle_alloc_error(layout);
        }

        // Initialize the buffer with green color
        let fb_slice = unsafe { core::slice::from_raw_parts_mut(fb_ptr, FRAME_BYTES) };
        let rgb565_slice =
            unsafe { core::slice::from_raw_parts_mut(fb_ptr as *mut Rgb565, LCD_BUFFER_SIZE) };

        // Fill with green color (0, 31, 0)
        for pixel in rgb565_slice.iter_mut() {
            *pixel = Rgb565::new(0, 31, 0);
        }

        let psram_buf: &'static mut [u8] = fb_slice;

        // Verify PSRAM buffer allocation and alignment
        let buf_ptr = psram_buf.as_ptr() as usize;
        info!("PSRAM buffer allocated at address: 0x{:08X}", buf_ptr);
        info!("PSRAM buffer length: {}", psram_buf.len());
        info!("PSRAM buffer alignment modulo 64: {}", buf_ptr % 64);
        assert!(buf_ptr % 64 == 0, "PSRAM buffer must be 64-byte aligned for DMA");

        // Publish PSRAM buffer pointer and len for app core
        unsafe {
            PSRAM_BUF_PTR = psram_buf.as_mut_ptr();
            PSRAM_BUF_LEN = psram_buf.len();
        }

        // Configure DMA buffer with proper burst configuration
        info!("=== DMA Buffer Configuration ===");
        let heap_before_dma = esp_alloc::HEAP.used();
        info!("Heap usage before DMA buffer creation: {} bytes", heap_before_dma);

        let dma_tx: DmaTxBuf = unsafe {
            DmaTxBuf::new_with_config(
                &mut *core::ptr::addr_of_mut!(TX_DESCRIPTORS),
                psram_buf,
                ExternalBurstConfig::Size64,
            )
            .unwrap()
        };

        let heap_after_dma = esp_alloc::HEAP.used();
        info!(
            "Heap usage after DMA buffer creation: {} bytes (delta: +{})",
            heap_after_dma,
            heap_after_dma.saturating_sub(heap_before_dma)
        );

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

        let dpi = Dpi::new(lcd_cam.lcd, peripherals.DMA_CH2, dpi_config)
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

        // Tell Slint the window dimensions match the display resolution
        let size = PhysicalSize::new(LCD_H_RES.into(), LCD_V_RES.into());
        self.window.borrow().as_ref().expect("Window adapter not created").set_size(size);

        // Initialize Embassy with both timers for multicore support
        info!("=== Embassy Initialization ===");
        let heap_before_embassy = esp_alloc::HEAP.used();
        info!("Heap usage before Embassy init: {} bytes", heap_before_embassy);

        let timg0 = TimerGroup::new(peripherals.TIMG0);
        let timer0: AnyTimer = timg0.timer0.into();
        let timg1 = TimerGroup::new(peripherals.TIMG1);
        let timer1: AnyTimer = timg1.timer0.into();

        info!("Initializing Embassy with dual timers for multicore support...");
        esp_hal_embassy::init([timer0, timer1]);

        let heap_after_embassy = esp_alloc::HEAP.used();
        info!(
            "Heap usage after Embassy init: {} bytes (delta: +{})",
            heap_after_embassy,
            heap_after_embassy.saturating_sub(heap_before_embassy)
        );

        // Signal that PSRAM is ready for the app core
        info!("Signaling PSRAM ready for app core...");
        PSRAM_READY.signal(());

        // Spawn app core for DMA display task (matching Conway)
        info!("=== App Core Startup ===");
        let heap_before_core = esp_alloc::HEAP.used();
        info!("Heap usage before app core startup: {} bytes", heap_before_core);

        let mut cpu_control = CpuControl::new(peripherals.CPU_CTRL);
        info!("Starting app core (Core 1) for DMA display task...");
        let _app_core = cpu_control.start_app_core(
            unsafe { &mut *core::ptr::addr_of_mut!(APP_CORE_STACK) },
            move || {
                info!("App core started! Initializing Embassy executor on Core 1...");

                // Initialize and run Embassy executor on app core
                static APP_EXECUTOR: StaticCell<Executor> = StaticCell::new();
                let executor = APP_EXECUTOR.init(Executor::new());
                info!("App core executor initialized, spawning DMA task...");

                executor.run(|spawner| match spawner.spawn(dma_display_task(dpi, dma_tx)) {
                    Ok(_) => info!("DMA display task spawned successfully on Core 1"),
                    Err(e) => error!("Failed to spawn DMA display task: {:?}", e),
                });
            },
        );

        // Initialize Embassy executor on main core for Slint rendering
        info!("=== Main Core Executor Setup ===");
        let heap_before_main_exec = esp_alloc::HEAP.used();
        info!("Heap usage before main executor init: {} bytes", heap_before_main_exec);

        static MAIN_EXECUTOR: StaticCell<Executor> = StaticCell::new();
        let executor = MAIN_EXECUTOR.init(Executor::new());
        info!("Main core executor initialized on Core 0");

        let window = self.window.borrow().as_ref().expect("Window not created").clone();

        let heap_before_rendering_spawn = esp_alloc::HEAP.used();
        info!(
            "Heap usage before Slint rendering task spawn: {} bytes",
            heap_before_rendering_spawn
        );

        executor.run(|spawner| {
            match spawner.spawn(slint_rendering_task(window, touch_controller)) {
                Ok(_) => info!("Slint rendering task spawned successfully on Core 0"),
                Err(e) => error!("Failed to spawn Slint rendering task: {:?}", e),
            }

            let heap_after_tasks = esp_alloc::HEAP.used();
            info!("Final heap usage after all tasks spawned: {} bytes", heap_after_tasks);
            info!("=== All tasks running, entering main executor loop ===");
        });
    }
}

#[embassy_executor::task]
async fn slint_rendering_task(
    window: Rc<slint::platform::software_renderer::MinimalSoftwareWindow>,
    mut touch_controller: TouchController,
) {
    info!("[CORE 1] Slint task starting, waiting for PSRAM ready signal...");

    // Wait for PSRAM to be ready
    PSRAM_READY.wait().await;
    info!("[CORE 1] PSRAM ready signal received!");

    // Get the PSRAM buffer
    let psram_ptr = unsafe { PSRAM_BUF_PTR };
    let psram_len = unsafe { PSRAM_BUF_LEN };

    if psram_ptr.is_null() || psram_len == 0 {
        error!(
            "[CORE 1] Invalid PSRAM buffer: ptr=0x{:08X}, len={}",
            psram_ptr as usize, psram_len
        );
        return;
    }

    let fb_slice: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(psram_ptr, psram_len) };

    info!(
        "[CORE 1] Slint task started on Core 1, PSRAM buffer at: 0x{:08X}, len: {}",
        psram_ptr as usize, psram_len
    );

    // Create pixel buffer for Slint rendering in PSRAM (using Box allocation)
    info!("[CORE 1] Creating pixel buffer in PSRAM...");
    let mut pixel_box: Box<[Rgb565Pixel; LCD_BUFFER_SIZE]> =
        Box::new([Rgb565Pixel(0); LCD_BUFFER_SIZE]);
    let pixel_buf: &mut [Rgb565Pixel] = &mut *pixel_box;
    info!("[CORE 1] Pixel buffer created in PSRAM, {} pixels", LCD_BUFFER_SIZE);

    // Signal that DMA is ready to be used now that everything is initialized
    info!("[CORE 1] Signaling DMA ready for Core 0...");
    DMA_READY.signal(());

    let mut ticker = Ticker::every(Duration::from_millis(200));
    let mut frame_counter = 0u32;
    let mut last_position = slint::LogicalPosition::default();
    let mut touch_down = false;

    info!("[CORE 1] Entering main rendering loop with Slint rendering and touch support...");

    loop {
        // Update Slint timers and animations
        slint::platform::update_timers_and_animations();

        // Poll touch controller for input events
        if let Ok(maybe_touch) = touch_controller.get_point0() {
            if let Some(sitronix_touch::Point { x: touchpad_x, y: touchpad_y }) = maybe_touch {
                last_position = slint::LogicalPosition::new(touchpad_x as f32, touchpad_y as f32);

                // Dispatch the pointer moved event
                window.dispatch_event(slint::platform::WindowEvent::PointerMoved {
                    position: last_position,
                });

                if !touch_down {
                    window.dispatch_event(slint::platform::WindowEvent::PointerPressed {
                        position: last_position,
                        button: slint::platform::PointerEventButton::Left,
                    });
                    if frame_counter % 60 == 0 {
                        debug!("[CORE 1] Touch pressed at ({}, {})", touchpad_x, touchpad_y);
                    }
                }

                touch_down = true;
            } else if touch_down {
                window.dispatch_event(slint::platform::WindowEvent::PointerReleased {
                    position: last_position,
                    button: slint::platform::PointerEventButton::Left,
                });
                window.dispatch_event(slint::platform::WindowEvent::PointerExited);
                touch_down = false;

                if frame_counter % 60 == 0 {
                    debug!("[CORE 1] Touch released");
                }
            }
        }

        // Use draw_if_needed to check if we need to render and get access to the renderer
        let rendered = window.draw_if_needed(|renderer| {
            // Render the Slint window to our pixel buffer
            // Slint will handle partial rendering and only update the areas that changed
            renderer.render(pixel_buf, LCD_H_RES as usize);

            if frame_counter % 60 == 0 {
                debug!("[CORE 1] Frame {} rendered by Slint", frame_counter);
            }
        });

        // Only convert and signal if something was actually rendered
        if rendered {
            // Convert pixel buffer to framebuffer
            for (i, px) in pixel_buf.iter().enumerate() {
                let fb_offset = i * 2;
                let [lo, hi] = px.0.to_le_bytes();
                fb_slice[fb_offset] = lo;
                fb_slice[fb_offset + 1] = hi;
            }

            if frame_counter % 60 == 0 {
                debug!("[CORE 1] Frame {} actually rendered by Slint", frame_counter);
            }
        } else {
            // Still convert buffer even if nothing was rendered (for first frame or fallback)
            for (i, px) in pixel_buf.iter().enumerate() {
                let fb_offset = i * 2;
                let [lo, hi] = px.0.to_le_bytes();
                fb_slice[fb_offset] = lo;
                fb_slice[fb_offset + 1] = hi;
            }

            if frame_counter % 60 == 0 {
                debug!("[CORE 1] Frame {} - no Slint rendering needed", frame_counter);
            }
        }

        // Signal that frame is ready for DMA
        FRAME_READY.signal(());

        frame_counter = frame_counter.wrapping_add(1);

        // Log periodic status
        if frame_counter % 60 == 0 {
            debug!("[CORE 1] Frame {}, continuing render loop...", frame_counter);
        }

        ticker.next().await;
    }
}

#[embassy_executor::task]
async fn dma_display_task(mut dpi: Dpi<'static, esp_hal::Blocking>, mut dma_tx: DmaTxBuf) {
    info!("[CORE 0] DMA task started on Core 0, waiting for DMA ready signal...");

    // Wait for DMA to be ready (all initialization complete)
    DMA_READY.wait().await;
    info!("[CORE 0] DMA ready signal received, starting DMA transfers!");

    // Stack monitoring removed for compilation compatibility

    let mut transfer_counter = 0u32;
    // Wait for frame to be ready
    FRAME_READY.wait().await;
    loop {
        transfer_counter = transfer_counter.wrapping_add(1);

        // Log periodic DMA status
        if transfer_counter % 60 == 0 {
            debug!("[CORE 0] DMA transfer {}, performing transfer...", transfer_counter);
        }

        // Set DMA transfer length (like Conway's working example)
        let frame_bytes = LCD_BUFFER_SIZE * 2;
        let dma_buf_len = dma_tx.as_slice().len();

        if transfer_counter % 60 == 0 {
            debug!(
                "[CORE 0] Setting DMA length: {} bytes, buffer len: {} bytes",
                frame_bytes, dma_buf_len
            );
        }

        if frame_bytes > dma_buf_len {
            error!("[CORE 0] Frame size {} exceeds DMA buffer size {}", frame_bytes, dma_buf_len);
            Timer::after(Duration::from_millis(10)).await;
            continue;
        }

        dma_tx.set_length(frame_bytes);

        // Perform DMA transfer
        match dpi.send(false, dma_tx) {
            Ok(xfer) => {
                let (res, new_dpi, new_dma_tx) = xfer.wait();
                dpi = new_dpi;
                dma_tx = new_dma_tx;
                if let Err(e) = res {
                    error!("[CORE 0] DMA transfer error: {:?}", e);
                } else if transfer_counter % 60 == 0 {
                    debug!("[CORE 0] DMA transfer {} completed successfully", transfer_counter);
                }
            }
            Err((e, new_dpi, new_dma_tx)) => {
                error!("[CORE 0] DMA send error: {:?}", e);
                dpi = new_dpi;
                dma_tx = new_dma_tx;

                // Add small delay on error to prevent spinning
                Timer::after(Duration::from_millis(100)).await;
            }
        }
    }
}
