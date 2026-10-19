// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
#[cfg(any(feature = "box-profile", feature = "box-parallel"))]
use core::sync::atomic::Ordering;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
#[path = "panel.rs"]
mod panel;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::DriveMode;
use esp_hal::time::Instant;
use esp_hal::{
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    i2c::master::I2c,
    spi::Mode as SpiMode,
    spi::master::{Config as SpiConfig, Spi},
    time::Rate,
};
use esp_println::logger::init_logger_from_env;
use gt911::Gt911Blocking;
#[cfg(feature = "box-imu")]
#[path = "background_job.rs"]
mod background_job;
#[cfg(feature = "box-imu")]
#[path = "imu.rs"]
mod imu;
#[cfg(feature = "box-imu")]
static INK_WORK: background_job::Queue = background_job::Queue::new();

#[cfg(feature = "box-imu")]
/// Run bounded particle steps on the display core alongside local computation.
/// Each step must return promptly so the display worker can meet its deadlines.
/// Both closures finish before this function returns.
pub fn with_ink_worker<F: FnMut() -> bool + Send, R>(work: &mut F, local: impl FnOnce() -> R) -> R {
    INK_WORK.scope(work, local, || esp_hal::rom::ets_delay_us(25))
}

#[cfg(feature = "box-imu")]
pub(super) fn work_ink() -> bool {
    INK_WORK.step()
}
#[cfg(feature = "box-imu")]
#[path = "tilt_fusion.rs"]
mod tilt_fusion;
#[cfg(feature = "box-imu")]
pub use imu::{set_active as set_ink_active, up as ink_up};
#[cfg(feature = "box-touch-worker")]
#[path = "touch_samples.rs"]
mod touch_samples;
#[cfg(feature = "box-touch-worker")]
#[path = "touch_worker.rs"]
mod touch_worker;
#[cfg(feature = "box-touch-worker")]
static TOUCH_SAMPLES: StaticCell<touch_samples::TouchSamples> = StaticCell::new();
use log::{error, info, warn};
use mipidsi::options::{ColorOrder, Orientation, Rotation};
use panel::{Panel, PanelInterface};
#[cfg(feature = "box-pie-fill")]
mod pie_fill;
#[cfg(feature = "box-pie-fill")]
mod pie_pixel;

#[path = "animation_frame.rs"]
mod animation_frame;
#[path = "native_buffer.rs"]
mod native_buffer;
#[cfg(feature = "box-profile")]
mod profile;
#[cfg(feature = "box-profile")]
use profile::FrameStats;
#[cfg(feature = "box-dma-backoff")]
#[path = "dma_bus.rs"]
mod dma_bus;
#[cfg(feature = "box-parallel")]
#[path = "frame_queue.rs"]
mod frame_queue;
#[cfg(feature = "box-parallel")]
static FRAME_QUEUE: StaticCell<frame_queue::FrameQueue> = StaticCell::new();
#[cfg(feature = "box-parallel")]
static QUEUE_REF: core::sync::atomic::AtomicPtr<frame_queue::FrameQueue> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());
fn now_us() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}

fn single_touch_event(
    position: slint::LogicalPosition,
    phase: TouchPhase,
    event_time: Option<i_slint_core::animations::Instant>,
) -> slint::platform::WindowEvent {
    slint::platform::WindowEvent::internal(i_slint_core::platform::InternalEvent::Touch {
        id: 0,
        position: i_slint_core::lengths::logical_point_from_api(position),
        phase,
        event_time,
        history: Default::default(),
    })
}

#[cfg(feature = "box-parallel")]
fn queue() -> &'static frame_queue::FrameQueue {
    // Published during init, before the Slint event loop or display core starts.
    unsafe { &*QUEUE_REF.load(core::sync::atomic::Ordering::Acquire) }
}
use crate::TouchPhase;
use slint::platform::WindowEvent;
use static_cell::StaticCell;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    esp_println::println!("Panic: {:?}", info);
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

type BoardI2c = I2c<'static, esp_hal::Blocking>;
#[cfg(not(feature = "box-parallel"))]
type BoardDisplay =
    mipidsi::Display<PanelInterface, mipidsi::models::ILI9342CRgb565, Output<'static>>;
#[cfg(feature = "box-parallel")]
type BoardDisplay = ();

/// Board hardware constructed in [`init`] and consumed by the event loop.
struct BoardState {
    #[cfg(feature = "box-parallel")]
    display_panel: Option<Panel>,
    #[cfg(feature = "box-parallel")]
    cpu_control: Option<esp_hal::peripherals::CPU_CTRL<'static>>,
    buffer_provider: DrawBuffer<'static, BoardDisplay>,
    touch: Gt911Blocking<BoardI2c>,
    touch_address: u8,
    i2c: BoardI2c,
    // Kept alive for the lifetime of the event loop.
    _backlight: Output<'static>,
    #[cfg(not(feature = "box-touch-irq"))]
    _int_pin: Output<'static>,
    #[cfg(feature = "box-touch-irq")]
    _int_pin: esp_hal::gpio::Flex<'static>,
}

struct EspBackend {
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow>>>,
    state: RefCell<Option<BoardState>>,
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
        EspBackend { window: RefCell::new(None), state: RefCell::new(None) }
    }
}

/// Initializes the heap, the board peripherals and sets the Slint platform.
pub fn init() {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_240MHz));
    init_logger_from_env();
    info!("Peripherals initialized");
    #[cfg(all(feature = "box-pie-fill", feature = "box-profile"))]
    if option_env!("BOX_PIE_FILL_SELF_TEST") == Some("1") {
        info!("BOX_PIE_FILL_SELF_TEST,cases={}", pie_fill::verify());
    }

    // Register the PSRAM heap before anything allocates. PSRAM is consumed here;
    // every other peripheral is used field by field below, so the whole
    // `Peripherals` struct is never stored and no `steal` is needed.
    #[cfg(not(feature = "box-parallel"))]
    esp_alloc::heap_allocator!(size: 96 * 1024);
    #[cfg(feature = "box-parallel")]
    esp_alloc::heap_allocator!(size: 48 * 1024);
    esp_alloc::psram_allocator!(
        peripherals.PSRAM,
        esp_hal::psram,
        esp_hal::psram::PsramConfig {
            mode: esp_hal::psram::PsramMode::OctalSpi,
            ..Default::default()
        }
    );

    let mut delay = Delay::new();

    // The following sequence is necessary to properly initialize touch on ESP32-S3-BOX-3
    // Based on issue from ESP-IDF: https://github.com/espressif/esp-bsp/issues/302#issuecomment-1971559689
    // Related code: https://github.com/espressif/esp-bsp/blob/30f0111a97b8fbe2efb7e58366fcf4d26b380f23/components/lcd_touch/esp_lcd_touch_gt911/esp_lcd_touch_gt911.c#L101-L133
    // --- Begin GT911 I²C Address Selection Sequence ---
    const ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS: u8 = 0x14;
    const ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS_BACKUP: u8 = 0x5D;

    // Our desired address.
    const DESIRED_ADDR: u8 = 0x14;
    // For desired address 0x14, assume the configuration’s reset level is false (i.e. 0 means active).
    let reset_level = Level::Low;

    // Configure the INT pin (GPIO3) as output; starting high because of internal pull-up.
    #[cfg(not(feature = "box-touch-irq"))]
    let mut int_pin = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    #[cfg(feature = "box-touch-irq")]
    let mut int_pin = {
        let mut pin = esp_hal::gpio::Flex::new(peripherals.GPIO3);
        pin.apply_output_config(&OutputConfig::default());
        pin.set_output_enable(true);
        pin.set_high();
        pin
    };
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
    .with_sio0(peripherals.GPIO6);

    #[cfg(feature = "box-dma")]
    let spi = {
        let (rx, rx_desc, tx, tx_desc) = esp_hal::dma_buffers!(4, 16 * 1024);
        let rx = esp_hal::dma::DmaRxBuf::new(rx_desc, rx).unwrap();
        let tx = esp_hal::dma::DmaTxBuf::new(tx_desc, tx).unwrap();
        let spi = spi.with_dma(peripherals.DMA_CH0);
        #[cfg(feature = "box-dma-backoff")]
        {
            dma_bus::DmaPanelBus::new(spi, rx, tx)
        }
        #[cfg(not(feature = "box-dma-backoff"))]
        {
            spi.with_buffers(rx, tx)
        }
    };

    // Display control pins.
    let dc = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
    let cs = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());

    let panel = Rc::new(RefCell::new(Panel::new(spi, cs, dc)));
    let di = PanelInterface(panel.clone());

    // Initialize the display.
    let display = mipidsi::Builder::new(mipidsi::models::ILI9342CRgb565, di)
        .reset_pin(rst)
        .orientation(Orientation::new().rotate(Rotation::Deg180))
        .color_order(ColorOrder::Bgr)
        .init(&mut delay)
        .unwrap();

    #[cfg(feature = "box-parallel")]
    {
        // The display worker's scanline gate requires these timings and
        // refresh direction in both production and profiling builds.
        let mut panel = panel.borrow_mut();
        panel.command(0xc8, &[0xff, 0x93, 0x42]).unwrap();
        panel.command(0xb1, &[0, 0x1b]).unwrap();
        panel.command(0xb5, &[28, 29, 0x0a, 0x14]).unwrap();
        panel.command(0x36, &[0xd8]).unwrap();
        delay.delay_ms(50);
    }

    // Set up the backlight pin.
    let mut backlight = Output::new(peripherals.GPIO47, Level::Low, OutputConfig::default());
    backlight.set_high();
    // --- End Display Initialization ---

    let mut i2c = I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO8)
    .with_scl(peripherals.GPIO18);

    #[cfg(feature = "box-touch-irq")]
    {
        // Address selection is complete. Release INT so the controller can
        // signal data; the default HAL handler latches and disables each edge.
        int_pin.set_output_enable(false);
        int_pin.apply_input_config(
            &esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        );
        int_pin.set_input_enable(true);
        int_pin.clear_interrupt();
        int_pin.listen(esp_hal::gpio::Event::AnyEdge);
    }
    // Initialize the touch driver.
    let mut touch_address = ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS;
    let mut touch = Gt911Blocking::new(touch_address);
    match touch.init(&mut i2c) {
        Ok(_) => info!("Touch initialized"),
        Err(e) => {
            warn!("Touch initialization failed: {:?}", e);
            let touch_fallback = Gt911Blocking::new(ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS_BACKUP);
            match touch_fallback.init(&mut i2c) {
                Ok(_) => {
                    info!("Touch initialized with backup address");
                    touch = touch_fallback;
                    touch_address = ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS_BACKUP;
                }
                Err(e) => error!("Touch initialization failed with backup address: {:?}", e),
            }
        }
    }

    // Prepare a draw buffer for the Slint software renderer. The line buffer must
    // outlive the platform, so it lives in a `StaticCell`.
    static LINE_BUFFER: StaticCell<[slint::platform::software_renderer::Rgb565Pixel; 320]> =
        StaticCell::new();
    #[cfg(feature = "box-frame-buffer")]
    #[cfg_attr(feature = "box-line-damage", repr(align(4)))]
    struct Framebuffer([slint::platform::software_renderer::Rgb565BigEndianPixel; 320 * 240]);
    #[cfg(feature = "box-frame-buffer")]
    static FRAME_BUFFER: static_cell::ConstStaticCell<Framebuffer> =
        static_cell::ConstStaticCell::new(Framebuffer(
            [slint::platform::software_renderer::Rgb565BigEndianPixel(0); 320 * 240],
        ));
    #[cfg(feature = "box-parallel")]
    let display_panel = {
        drop(display);
        Rc::try_unwrap(panel).ok().unwrap().into_inner()
    };
    #[cfg(feature = "box-parallel")]
    let display = ();
    #[cfg(feature = "box-parallel")]
    {
        static PACKET: static_cell::ConstStaticCell<[u8; 64 * 1024]> =
            static_cell::ConstStaticCell::new([0; 64 * 1024]);
        let queue = FRAME_QUEUE.init(frame_queue::FrameQueue::new(PACKET.take()));
        QUEUE_REF.store(queue, core::sync::atomic::Ordering::Release);
    }
    let buffer_provider = DrawBuffer {
        display,
        #[cfg(all(feature = "box-frame-buffer", not(feature = "box-parallel")))]
        panel: panel.clone(),
        #[cfg(feature = "box-frame-buffer")]
        frame_buffer: &mut FRAME_BUFFER.take().0,
        #[cfg(feature = "box-line-damage")]
        first_line_frame: true,
        #[cfg(feature = "box-line-damage")]
        line_damage: Box::new(frame_queue::pixel_damage::Plan::new()),
        buffer: LINE_BUFFER.init([slint::platform::software_renderer::Rgb565Pixel(0); 320]),
        #[cfg(feature = "box-profile")]
        stats: FrameStats::default(),
    };

    slint::platform::set_platform(Box::new(EspBackend {
        window: RefCell::new(None),
        state: RefCell::new(Some(BoardState {
            #[cfg(feature = "box-parallel")]
            display_panel: Some(display_panel),
            #[cfg(feature = "box-parallel")]
            cpu_control: Some(peripherals.CPU_CTRL),
            buffer_provider,
            touch,
            touch_address,
            i2c,
            _backlight: backlight,
            _int_pin: int_pin,
        })),
    }))
    .expect("backend already initialized");
}

impl EspBackend {
    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let BoardState {
            mut buffer_provider,
            touch,
            touch_address,
            mut i2c,
            _backlight,
            _int_pin,
            #[cfg(feature = "box-parallel")]
            cpu_control,
            #[cfg(feature = "box-parallel")]
            display_panel,
        } = self.state.borrow_mut().take().expect("event loop already running");

        // Update the Slint window size from the display.
        let size = slint::PhysicalSize::new(320, 240);
        self.window.borrow().as_ref().unwrap().set_size(size);

        // Variable to track the last touch position.
        #[cfg(feature = "box-parallel")]
        let mut cpu_control = esp_hal::system::CpuControl::new(cpu_control.unwrap());
        #[cfg(feature = "box-parallel")]
        static DISPLAY_STACK: static_cell::ConstStaticCell<esp_hal::system::Stack<16_384>> =
            static_cell::ConstStaticCell::new(esp_hal::system::Stack::new());
        #[cfg(feature = "box-parallel")]
        let display_panel = display_panel.unwrap();
        #[cfg(feature = "box-touch-worker")]
        let (touch_producer, touch_consumer) =
            TOUCH_SAMPLES.init(touch_samples::TouchSamples::new()).split();
        #[cfg(feature = "box-touch-worker")]
        let mut touch_receiver = touch_samples::TouchReceiver::new(touch_consumer);
        #[cfg(feature = "box-touch-worker")]
        let touch_worker =
            touch_worker::TouchWorker::new(i2c, touch, touch_address, touch_producer);
        #[cfg(all(feature = "box-parallel", not(feature = "box-touch-worker")))]
        let _display_guard = cpu_control
            .start_app_core(DISPLAY_STACK.take(), move || queue().display(display_panel))
            .unwrap();
        #[cfg(feature = "box-touch-worker")]
        let _display_guard = cpu_control
            .start_app_core(DISPLAY_STACK.take(), move || {
                queue().display_with_touch(display_panel, touch_worker)
            })
            .unwrap();
        #[cfg(feature = "box-parallel")]
        let mut last_tick = u32::MAX;
        #[cfg(feature = "box-parallel")]
        let mut animation_clock = animation_frame::FrameClock::default();
        let mut last_touch = None;
        #[cfg(all(feature = "box-touch-irq", not(feature = "box-touch-worker")))]
        let (mut touch_probe_at, mut touch_retry_at, mut touch_failures) = (0u64, 0u64, 0u32);
        #[cfg(feature = "box-touch-irq")]
        let mut touch_irq = _int_pin;
        #[cfg(feature = "box-profile")]
        let mut profile = profile::Profile::new();
        loop {
            #[cfg(feature = "box-touch-worker")]
            if touch_irq.is_interrupt_set() {
                touch_irq.clear_interrupt();
                touch_irq.listen(esp_hal::gpio::Event::AnyEdge);
                touch_worker::REQUEST.store(true, Ordering::Release);
            }
            #[cfg(feature = "box-parallel")]
            {
                if queue().tick.load(Ordering::Acquire) == last_tick {
                    esp_hal::rom::ets_delay_us(100);
                    continue;
                }
                let (tick, _blank) = queue().phase_snapshot();
                last_tick = tick;
                #[cfg(feature = "box-profile")]
                {
                    queue().acquire_us.store(now_us() as u32, Ordering::Release);
                    queue().acquire_blank_us.store(_blank, Ordering::Release);
                }
                #[cfg(feature = "box-profile")]
                queue().render_tick.store(tick, Ordering::Release);
            }
            #[cfg(feature = "box-parallel")]
            if let Some(frame) = animation_clock.acquire(last_tick) {
                animation_frame::invoke(frame);
            }
            slint::platform::update_timers_and_animations();
            #[cfg(feature = "box-profile")]
            profile.select();
            if let Some(window) = self.window.borrow().clone() {
                #[cfg(not(feature = "box-touch-irq"))]
                let touch_result = touch.get_touch(&mut i2c);
                #[cfg(all(feature = "box-touch-irq", not(feature = "box-touch-worker")))]
                let touch_result = {
                    let now = now_us();
                    let pending = touch_irq.is_interrupt_set();
                    if now < touch_retry_at
                        || (!pending && last_touch.is_none() && now < touch_probe_at)
                    {
                        Err(gt911::Error::NotReady)
                    } else {
                        // Rearm before the transaction so a pulse during I²C
                        // remains latched for the next frame. Poll held fingers
                        // each tick for movement/release, and idle every 100 ms
                        // as recovery for a lost interrupt.
                        touch_irq.clear_interrupt();
                        touch_irq.listen(esp_hal::gpio::Event::AnyEdge);
                        touch_probe_at = now + 100_000;
                        let result = touch.get_touch(&mut i2c);
                        match &result {
                            Ok(_) | Err(gt911::Error::NotReady) => touch_failures = 0,
                            Err(_) => {
                                touch_failures += 1;
                                if touch_failures >= 3 {
                                    touch_retry_at = now + 500_000;
                                    touch_failures = 0;
                                }
                            }
                        }
                        result
                    }
                };
                let mut sample_time = None;
                #[cfg(feature = "box-touch-worker")]
                let touch_result: Result<
                    Option<gt911::Point>,
                    gt911::Error<esp_hal::i2c::master::Error>,
                > = {
                    let pending = touch_irq.is_interrupt_set();
                    if pending {
                        touch_irq.clear_interrupt();
                        touch_irq.listen(esp_hal::gpio::Event::AnyEdge);
                        touch_worker::REQUEST.store(true, core::sync::atomic::Ordering::Release);
                    }
                    if let Some(sample) = touch_receiver.next(last_touch.is_some()) {
                        // Match the platform's millisecond animation clock. This is
                        // controller-read completion time, not sensor acquisition time.
                        sample_time = Some(i_slint_core::animations::Instant::from_millis(
                            sample.completed_us / 1000,
                        ));
                        match sample.state {
                            touch_samples::TouchState::Contact { x, y } => {
                                Ok(Some(gt911::Point { track_id: 0, x, y, area: 0 }))
                            }
                            touch_samples::TouchState::Released => Ok(None),
                        }
                    } else {
                        Err(gt911::Error::NotReady)
                    }
                };

                match touch_result {
                    Ok(Some(point)) => {
                        let pos = slint::PhysicalPosition::new(point.x as i32, point.y as i32)
                            .to_logical(window.scale_factor());
                        let phase = if let Some(previous) = last_touch.replace(pos) {
                            (previous != pos).then_some(TouchPhase::Moved)
                        } else {
                            Some(TouchPhase::Started)
                        };
                        if let Some(phase) = phase {
                            window.dispatch_event_with_result(single_touch_event(
                                pos,
                                phase,
                                sample_time,
                            ))?;
                        }
                    }
                    Ok(None) => {
                        if let Some(pos) = last_touch.take() {
                            window.dispatch_event_with_result(single_touch_event(
                                pos,
                                TouchPhase::Ended,
                                sample_time,
                            ))?;
                        }
                    }
                    Err(_) => {}
                }
                #[cfg(feature = "box-profile")]
                {
                    buffer_provider.stats = FrameStats::default();
                    if profile.capture_requested() {
                        window.request_redraw();
                    }
                }
                #[cfg(feature = "box-profile")]
                let draw_start = now_us();
                let rendered = window.draw_if_needed(|renderer| {
                    #[cfg(not(feature = "box-frame-buffer"))]
                    renderer.render_by_line(&mut buffer_provider);
                    #[cfg(feature = "box-frame-buffer")]
                    buffer_provider.render(renderer);
                    #[cfg(feature = "box-profile")]
                    if profile.capture_requested() {
                        profile.capture(renderer, buffer_provider.frame_buffer);
                        window.request_redraw();
                    }
                });
                #[cfg(feature = "box-profile")]
                profile.frame(draw_start, now_us() - draw_start, rendered, buffer_provider.stats);
                #[cfg(feature = "box-parallel")]
                if rendered {
                    let (current, blank) = queue().phase_snapshot();
                    if let Some(recovered) = frame_queue::phase_clock::recovery_tick(
                        last_tick,
                        current,
                        blank,
                        now_us() as u32,
                    ) {
                        last_tick = recovered;
                    }
                }
            }
        }
    }
}
/// Provides a draw buffer for the MinimalSoftwareWindow renderer.
struct DrawBuffer<'a, Display> {
    display: Display,
    #[cfg(feature = "box-line-damage")]
    first_line_frame: bool,
    #[cfg(feature = "box-line-damage")]
    line_damage: Box<frame_queue::pixel_damage::Plan>,
    #[cfg(all(feature = "box-frame-buffer", not(feature = "box-parallel")))]
    panel: Rc<RefCell<Panel>>,
    #[cfg(feature = "box-frame-buffer")]
    frame_buffer: &'a mut [slint::platform::software_renderer::Rgb565BigEndianPixel],
    buffer: &'a mut [slint::platform::software_renderer::Rgb565Pixel],
    #[cfg(feature = "box-profile")]
    stats: FrameStats,
}

#[cfg(feature = "box-pie-fill")]
type LinePixel = pie_pixel::Pixel;
#[cfg(all(feature = "box-line-damage", not(feature = "box-pie-fill")))]
type LinePixel = slint::platform::software_renderer::Rgb565BigEndianPixel;

#[cfg(feature = "box-line-damage")]
fn canonical_line(
    pixels: &[LinePixel],
) -> &[slint::platform::software_renderer::Rgb565BigEndianPixel] {
    #[cfg(feature = "box-pie-fill")]
    {
        pie_pixel::as_be(pixels)
    }
    #[cfg(not(feature = "box-pie-fill"))]
    {
        pixels
    }
}

#[cfg(feature = "box-line-damage")]
struct FramebufferLineBuffer<'a> {
    scratch: &'a mut [LinePixel],
    framebuffer: &'a mut [slint::platform::software_renderer::Rgb565BigEndianPixel],
    damage: &'a mut frame_queue::pixel_damage::Plan,
}

#[cfg(feature = "box-line-damage")]
impl slint::platform::software_renderer::LineBufferProvider for &mut FramebufferLineBuffer<'_> {
    type TargetPixel = LinePixel;
    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        let buffer = &mut self.scratch[range.clone()];
        render_fn(buffer);
        let old = &self.framebuffer[line * 320 + range.start..line * 320 + range.end];
        let current =
            unsafe { core::slice::from_raw_parts(buffer.as_ptr().cast::<u16>(), buffer.len()) };
        let previous =
            unsafe { core::slice::from_raw_parts(old.as_ptr().cast::<u16>(), old.len()) };
        if let Some(span) =
            frame_queue::pixel_damage::changed_line(current, previous, range.start, line)
        {
            let bounds = span.copy_bounds();
            self.damage.add(span);
            for (left, right) in bounds {
                if left < right {
                    self.framebuffer[line * 320 + left..line * 320 + right]
                        .copy_from_slice(canonical_line(&self.scratch[left..right]));
                }
            }
        }
    }
}

#[cfg(feature = "box-frame-buffer")]
fn render_full_frame(
    renderer: &slint::platform::software_renderer::SoftwareRenderer,
    pixels: &mut [slint::platform::software_renderer::Rgb565BigEndianPixel],
) -> slint::platform::software_renderer::PhysicalRegion {
    #[cfg(feature = "box-pie-fill")]
    let pixels = pie_pixel::as_mut(pixels);
    renderer.render_into_buffer(&mut native_buffer::FrameBuffer::new(pixels, 320))
}

#[cfg(feature = "box-frame-buffer")]
impl<Display> DrawBuffer<'_, Display> {
    fn render(&mut self, renderer: &slint::platform::software_renderer::SoftwareRenderer) {
        #[cfg(feature = "box-profile")]
        let start = now_us();
        #[cfg(not(feature = "box-line-damage"))]
        let region = render_full_frame(renderer, self.frame_buffer);
        #[cfg(feature = "box-line-damage")]
        let line_damage_enabled = LINE_DAMAGE_ENABLED.load(core::sync::atomic::Ordering::Relaxed);
        #[cfg(feature = "box-line-damage")]
        let region = if line_damage_enabled && !self.first_line_frame {
            self.line_damage.reset();
            #[repr(align(4))]
            struct Scanline([LinePixel; 320]);
            let mut scratch = Scanline([LinePixel::default(); 320]);
            let mut provider = FramebufferLineBuffer {
                scratch: &mut scratch.0,
                framebuffer: self.frame_buffer,
                #[cfg(feature = "box-line-damage")]
                damage: &mut self.line_damage,
            };
            let region = renderer.render_by_line(&mut provider);
            provider.damage.fit_packet(32 * 1024);
            region
        } else {
            render_full_frame(renderer, self.frame_buffer)
        };
        #[cfg(feature = "box-profile")]
        {
            self.stats.raster_us += now_us() - start;
        }
        #[cfg(feature = "box-profile")]
        let start = now_us();
        #[cfg(all(feature = "box-parallel", not(feature = "box-line-damage")))]
        queue().publish(self.frame_buffer, &region);
        #[cfg(feature = "box-line-damage")]
        if !self.first_line_frame && line_damage_enabled {
            queue().publish_damage(self.frame_buffer, &self.line_damage);
        } else {
            queue().publish(self.frame_buffer, &region);
            self.first_line_frame = region.iter().next().is_none();
        }
        #[cfg(not(feature = "box-parallel"))]
        for (origin, size) in region.iter() {
            self.panel
                .borrow_mut()
                .rectangle(
                    origin.x as usize,
                    origin.y as usize,
                    size.width as usize,
                    size.height as usize,
                    self.frame_buffer,
                )
                .unwrap();
        }

        #[cfg(feature = "box-profile")]
        {
            #[cfg(feature = "box-parallel")]
            {
                self.stats.queue_us += now_us() - start;
            }
        }
    }
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

        // Update the display with the rendered line.
        self.display
            .set_pixels(
                range.start as u16,
                line as u16,
                (range.end - 1) as u16,
                line as u16,
                buffer
                    .iter()
                    .map(|x| embedded_graphics_core::pixelcolor::raw::RawU16::new(x.0).into()),
            )
            .unwrap();
    }
}

#[cfg(feature = "box-line-damage")]
static LINE_DAMAGE_ENABLED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(true);

/// Select exact scanline damage for content that benefits from smaller display writes.
/// When disabled, the framebuffer renderer and its geometric dirty regions are used.
/// Call this on the Slint thread before drawing the next frame.
#[cfg(feature = "box-parallel")]
pub fn set_line_damage_enabled(enabled: bool) {
    #[cfg(feature = "box-line-damage")]
    LINE_DAMAGE_ENABLED.store(enabled, core::sync::atomic::Ordering::Relaxed);
    #[cfg(not(feature = "box-line-damage"))]
    let _ = enabled;
}

/// Register another animation on the Slint thread, preserving callback order.
#[cfg(feature = "box-parallel")]
pub fn add_animation_frame_handler(callback: impl FnMut(u64) + 'static) {
    animation_frame::add_handler(callback);
}

#[cfg(feature = "box-profile")]
pub use profile::{profile_ink_frame, profile_mode, profile_scroll_y};
#[cfg(feature = "box-profile")]
pub fn profile_animation_tick() -> u64 {
    queue().render_tick.load(Ordering::Acquire) as u64 + 1
}
#[cfg(feature = "box-profile")]
pub fn profile_now_us() -> u64 {
    now_us()
}
#[cfg(feature = "box-profile")]
pub fn profile_capture() {
    profile::request_capture();
}
