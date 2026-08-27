// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cSpell: ignore eeid multicore TIMG
//! Board support for ESoPe-SLD-C-W-S3 board with display and touch controller support.

extern crate alloc;

// Import embedded_graphics_core types
use embedded_graphics_core::pixelcolor::Rgb565;
use embedded_graphics_framebuf::backends::FrameBufferBackend;
// --- Slint platform integration imports ---
use slint::PhysicalSize;
use slint::platform::software_renderer::Rgb565Pixel;

use alloc::alloc::{alloc, handle_alloc_error};
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::alloc::Layout;
use core::cell::RefCell;

use eeprom24x::{Eeprom24x, SlaveAddr};
use embedded_hal_bus::i2c::RefCellDevice;
use esp_hal::clock::CpuClock;
use esp_hal::dma::ExternalBurstConfig;
use esp_hal::dma::{CHUNK_SIZE, DmaDescriptor, DmaTxBuf};
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::I2c;
use esp_hal::lcd_cam::{
    LcdCam,
    lcd::{
        ClockMode, Phase, Polarity,
        dpi::{Config as DpiConfig, Dpi, Format, FrameTiming},
    },
};

// Type alias for I2C device to simplify signatures
type I2cDevice = RefCellDevice<'static, esp_hal::i2c::master::I2c<'static, esp_hal::Blocking>>;
type TouchController = sitronix_touch::TouchIC<I2cDevice>;
use esp_hal::Config as HalConfig;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::peripherals::{CPU_CTRL, TIMG0};
use esp_hal::system::Stack;
use esp_hal::time::{Instant, Rate};
use esp_hal::timer::timg::TimerGroup;
use esp_println::logger::init_logger_from_env;
use log::{debug, error, info};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Ticker, Timer};
use esp_rtos::embassy::Executor;
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

esp_bootloader_esp_idf::esp_app_desc!();

#[unsafe(link_section = ".dma")]
static mut TX_DESCRIPTORS: [DmaDescriptor; MAX_NUM_DMA_DESC] =
    [DmaDescriptor::EMPTY; MAX_NUM_DMA_DESC];

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!("PANIC: {}", info);
    loop {}
}

/// Board hardware constructed in [`init`] and consumed by the event loop, which
/// starts the esp-rtos scheduler and hands the drivers to the embassy tasks.
struct BoardState {
    dpi: Dpi<'static, esp_hal::Blocking>,
    dma_tx: DmaTxBuf,
    touch_controller: TouchController,
    timer_group: TimerGroup<'static, TIMG0<'static>>,
    sw_ints: SoftwareInterruptControl<'static>,
    cpu_ctrl: CPU_CTRL<'static>,
    // Kept alive for the lifetime of the event loop.
    _panel_enable: Output<'static>,
    _backlight: Output<'static>,
    _touch_reset: Output<'static>,
}

struct EspBackend {
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow>>>,
    state: RefCell<Option<BoardState>>,
}

impl Default for EspBackend {
    fn default() -> Self {
        EspBackend { window: RefCell::new(None), state: RefCell::new(None) }
    }
}

/// Initialize the heap, the board peripherals and set the Slint platform.
pub fn init() {
    let config = HalConfig::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    init_logger_from_env();
    info!("=== ESP32-S3 ESoPe Board Initialization Starting ===");

    // Register the PSRAM heap before anything allocates. PSRAM is consumed here;
    // every other peripheral is used field by field below, so the whole
    // `Peripherals` struct is never stored and no `steal` is needed.
    esp_alloc::psram_allocator!(
        peripherals.PSRAM,
        esp_hal::psram,
        esp_hal::psram::PsramConfig {
            mode: esp_hal::psram::PsramMode::OctalSpi,
            ..Default::default()
        }
    );
    info!("PSRAM allocator initialized");

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

    let touch_reset = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

    // Allocate framebuffer in PSRAM with 64-byte alignment for DMA
    const FRAME_BYTES: usize = LCD_BUFFER_SIZE * 2;
    let layout =
        Layout::from_size_align(FRAME_BYTES, 64).expect("Failed to create layout for framebuffer");
    let fb_ptr = unsafe { alloc(layout) };
    if fb_ptr.is_null() {
        handle_alloc_error(layout);
    }

    // Initialize the buffer with green color
    let fb_slice = unsafe { core::slice::from_raw_parts_mut(fb_ptr, FRAME_BYTES) };
    let rgb565_slice =
        unsafe { core::slice::from_raw_parts_mut(fb_ptr as *mut Rgb565, LCD_BUFFER_SIZE) };
    for pixel in rgb565_slice.iter_mut() {
        *pixel = Rgb565::new(0, 31, 0);
    }

    let psram_buf: &'static mut [u8] = fb_slice;
    let buf_ptr = psram_buf.as_ptr() as usize;
    assert!(buf_ptr % 64 == 0, "PSRAM buffer must be 64-byte aligned for DMA");

    // Publish PSRAM buffer pointer and len for app core
    unsafe {
        PSRAM_BUF_PTR = psram_buf.as_mut_ptr();
        PSRAM_BUF_LEN = psram_buf.len();
    }

    // Configure DMA buffer with proper burst configuration
    let dma_tx: DmaTxBuf = unsafe {
        DmaTxBuf::new_with_config(
            &mut *core::ptr::addr_of_mut!(TX_DESCRIPTORS),
            psram_buf,
            ExternalBurstConfig::Size64,
        )
        .unwrap()
    };

    // Initialize LCD DPI interface
    let lcd_cam = LcdCam::new(peripherals.LCD_CAM);

    // Read configuration from EEPROM
    let pclk_hz = ((eeid[12] as u32) * 1_000_000 + (eeid[13] as u32) * 100_000).min(13_600_000);
    let flags = eeid[25];
    let hsync_idle_low = (flags & 0x01) != 0;
    let vsync_idle_low = (flags & 0x02) != 0;
    let de_idle_high = (flags & 0x04) != 0;
    let pclk_active_neg = (flags & 0x20) != 0;

    let dpi_config = DpiConfig::default()
        .with_clock_mode(ClockMode {
            polarity: if pclk_active_neg { Polarity::IdleHigh } else { Polarity::IdleLow },
            phase: if pclk_active_neg { Phase::ShiftHigh } else { Phase::ShiftLow },
        })
        .with_frequency(Rate::from_hz(pclk_hz))
        .with_format(Format { enable_2byte_mode: true, ..Default::default() })
        .with_timing(FrameTiming {
            horizontal_active_width: 320,
            horizontal_total_width: 320 + 4 + 43 + 79 + 8,
            horizontal_blank_front_porch: 79 + 8,
            vertical_active_height: 240,
            vertical_total_height: 240 + 4 + 12 + 16,
            vertical_blank_front_porch: 16,
            hsync_width: 4,
            vsync_width: 4,
            hsync_position: 43 + 4,
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

    slint::platform::set_platform(Box::new(EspBackend {
        window: RefCell::new(None),
        state: RefCell::new(Some(BoardState {
            dpi,
            dma_tx,
            touch_controller,
            timer_group: TimerGroup::new(peripherals.TIMG0),
            sw_ints: SoftwareInterruptControl::new(peripherals.SW_INTERRUPT),
            cpu_ctrl: peripherals.CPU_CTRL,
            _panel_enable: panel_enable,
            _backlight: backlight,
            _touch_reset: touch_reset,
        })),
    }))
    .expect("Slint platform already initialized");
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

        let BoardState {
            dpi,
            dma_tx,
            touch_controller,
            timer_group,
            sw_ints,
            cpu_ctrl,
            _panel_enable,
            _backlight,
            _touch_reset,
        } = self.state.borrow_mut().take().expect("event loop already running");

        // Tell Slint the window dimensions match the display resolution
        let size = PhysicalSize::new(LCD_H_RES.into(), LCD_V_RES.into());
        self.window.borrow().as_ref().expect("Window adapter not created").set_size(size);

        // Start the esp-rtos scheduler that drives the embassy executors on both cores.
        info!("=== esp-rtos Initialization ===");
        let heap_before_rtos = esp_alloc::HEAP.used();
        info!("Heap usage before esp-rtos init: {} bytes", heap_before_rtos);

        info!("Starting esp-rtos scheduler on the main core...");
        esp_rtos::start(timer_group.timer0, sw_ints.software_interrupt0);

        let heap_after_rtos = esp_alloc::HEAP.used();
        info!(
            "Heap usage after esp-rtos init: {} bytes (delta: +{})",
            heap_after_rtos,
            heap_after_rtos.saturating_sub(heap_before_rtos)
        );

        // Signal that PSRAM is ready for the app core
        info!("Signaling PSRAM ready for app core...");
        PSRAM_READY.signal(());

        // Spawn app core for DMA display task (matching Conway)
        info!("=== App Core Startup ===");
        let heap_before_core = esp_alloc::HEAP.used();
        info!("Heap usage before app core startup: {} bytes", heap_before_core);

        info!("Starting app core (Core 1) for DMA display task...");
        esp_rtos::start_second_core(
            cpu_ctrl,
            sw_ints.software_interrupt1,
            unsafe { &mut *core::ptr::addr_of_mut!(APP_CORE_STACK) },
            move || {
                info!("App core started! Initializing Embassy executor on Core 1...");

                // Initialize and run Embassy executor on app core
                static APP_EXECUTOR: StaticCell<Executor> = StaticCell::new();
                let executor = APP_EXECUTOR.init(Executor::new());
                info!("App core executor initialized, spawning DMA task...");

                executor.run(|spawner| match dma_display_task(dpi, dma_tx) {
                    Ok(token) => {
                        spawner.spawn(token);
                        info!("DMA display task spawned successfully on Core 1");
                    }
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
            match slint_rendering_task(window, touch_controller) {
                Ok(token) => {
                    spawner.spawn(token);
                    info!("Slint rendering task spawned successfully on Core 0");
                }
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
