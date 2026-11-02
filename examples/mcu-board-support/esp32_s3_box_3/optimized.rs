// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::{boxed::Box, rc::Rc};
use core::{cell::RefCell, sync::atomic::Ordering};
use embedded_hal::delay::DelayNs;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{DriveMode, Level, Output, OutputConfig},
    i2c::master::I2c,
    spi::{
        Mode as SpiMode,
        master::{Config as SpiConfig, Spi},
    },
    time::{Instant, Rate},
};
use esp_println::logger::init_logger_from_env;
use gt911::Gt911Blocking;
use i_slint_core::input::TouchPhase;
use log::{error, info, warn};
use mipidsi::options::{ColorOrder, Orientation, Rotation};
use static_cell::StaticCell;

#[path = "dma_bus.rs"]
mod dma_bus;
#[path = "frame_queue.rs"]
mod frame_queue;
#[path = "native_buffer.rs"]
mod native_buffer;
#[path = "panel.rs"]
mod panel;
mod pie_fill;
mod pie_pixel;
#[path = "touch_samples.rs"]
mod touch_samples;
#[path = "touch_worker.rs"]
mod touch_worker;
use panel::{Panel, PanelInterface};

static TOUCH_SAMPLES: StaticCell<touch_samples::TouchSamples> = StaticCell::new();
static FRAME_QUEUE: StaticCell<frame_queue::FrameQueue> = StaticCell::new();
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

fn queue() -> &'static frame_queue::FrameQueue {
    unsafe { &*QUEUE_REF.load(core::sync::atomic::Ordering::Acquire) }
}

esp_bootloader_esp_idf::esp_app_desc!();

type BoardI2c = I2c<'static, esp_hal::Blocking>;

struct BoardState {
    display_panel: Option<Panel>,
    cpu_control: Option<esp_hal::peripherals::CPU_CTRL<'static>>,
    buffer_provider: DrawBuffer<'static>,
    touch: Gt911Blocking<BoardI2c>,
    i2c: BoardI2c,
    _backlight: Output<'static>,

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
        window.set_size(slint::PhysicalSize::new(320, 240));
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
    init_board();
}

pub fn init_with_host_link() -> impl crate::HostLink {
    crate::esp32_s3_usb_serial::host_link(init_board(), "ESP32-S3-BOX-3")
}

fn init_board() -> esp_hal::peripherals::USB_DEVICE<'static> {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_240MHz));
    init_logger_from_env();
    info!("Peripherals initialized");

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

    // Based on issue from ESP-IDF: https://github.com/espressif/esp-bsp/issues/302#issuecomment-1971559689
    // Related code: https://github.com/espressif/esp-bsp/blob/30f0111a97b8fbe2efb7e58366fcf4d26b380f23/components/lcd_touch/esp_lcd_touch_gt911/esp_lcd_touch_gt911.c#L101-L133
    const ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS: u8 = 0x14;
    const ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS_BACKUP: u8 = 0x5D;

    const DESIRED_ADDR: u8 = 0x14;
    let reset_level = Level::Low;

    let mut int_pin = {
        let mut pin = esp_hal::gpio::Flex::new(peripherals.GPIO3);
        pin.apply_output_config(&OutputConfig::default());
        pin.set_output_enable(true);
        pin.set_high();
        pin
    };
    int_pin.set_low();
    delay.delay_ms(10);
    int_pin.set_low();
    delay.delay_ms(1);

    let mut rst = Output::new(
        peripherals.GPIO48,
        Level::Low,
        OutputConfig::default().with_drive_mode(DriveMode::OpenDrain),
    );

    rst.set_level(reset_level);
    int_pin.set_low();
    delay.delay_ms(10);

    let gpio_level = if DESIRED_ADDR == ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS {
        Level::Low
    } else if DESIRED_ADDR == ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS_BACKUP {
        Level::High
    } else {
        Level::Low
    };
    int_pin.set_level(gpio_level);
    delay.delay_ms(1);

    rst.set_level(!reset_level);
    delay.delay_ms(10);
    delay.delay_ms(50);

    let spi = Spi::<esp_hal::Blocking>::new(
        peripherals.SPI2,
        SpiConfig::default().with_frequency(Rate::from_mhz(40)).with_mode(SpiMode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO7)
    .with_sio0(peripherals.GPIO6);

    let spi = {
        let (rx, rx_desc, tx, tx_desc) = esp_hal::dma_buffers!(4, 8 * 1024);
        let rx = esp_hal::dma::DmaRxBuf::new(rx_desc, rx).unwrap();
        let tx = esp_hal::dma::DmaTxBuf::new(tx_desc, tx).unwrap();
        let spare_tx = esp_hal::dma_tx_buffer!(8 * 1024).unwrap();
        let spi = spi.with_dma(peripherals.DMA_CH0);

        dma_bus::DmaPanelBus::new(spi, rx, tx, spare_tx)
    };

    let dc = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
    let cs = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());

    let panel = Rc::new(RefCell::new(Panel::new(spi, cs, dc)));
    let di = PanelInterface(panel.clone());

    let display = mipidsi::Builder::new(mipidsi::models::ILI9342CRgb565, di)
        .reset_pin(rst)
        .orientation(Orientation::new().rotate(Rotation::Deg180))
        .color_order(ColorOrder::Bgr)
        .init(&mut delay)
        .unwrap();

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

    let mut backlight = Output::new(peripherals.GPIO47, Level::Low, OutputConfig::default());
    backlight.set_high();

    let mut i2c = I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO8)
    .with_scl(peripherals.GPIO18);

    {
        int_pin.set_output_enable(false);
        int_pin.apply_input_config(
            &esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        );
        int_pin.set_input_enable(true);
        int_pin.clear_interrupt();
        int_pin.listen(esp_hal::gpio::Event::AnyEdge);
    }
    let touch_address = ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS;
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
                }
                Err(e) => error!("Touch initialization failed with backup address: {:?}", e),
            }
        }
    }

    #[repr(align(4))]
    struct Framebuffer([slint::platform::software_renderer::Rgb565BigEndianPixel; 320 * 240]);

    static FRAME_BUFFER: static_cell::ConstStaticCell<Framebuffer> =
        static_cell::ConstStaticCell::new(Framebuffer(
            [slint::platform::software_renderer::Rgb565BigEndianPixel(0); 320 * 240],
        ));

    let display_panel = {
        drop(display);
        Rc::try_unwrap(panel).ok().unwrap().into_inner()
    };

    {
        static PACKET: static_cell::ConstStaticCell<[u8; frame_queue::INTERNAL_PACKET_BYTES]> =
            static_cell::ConstStaticCell::new([0; frame_queue::INTERNAL_PACKET_BYTES]);
        let queue = FRAME_QUEUE.init(frame_queue::FrameQueue::new(PACKET.take()));
        QUEUE_REF.store(queue, core::sync::atomic::Ordering::Release);
    }
    let buffer_provider = DrawBuffer {
        frame_buffer: &mut FRAME_BUFFER.take().0,

        first_line_frame: true,
        line_damage: Box::new(frame_queue::pixel_damage::Plan::new()),
    };

    slint::platform::set_platform(Box::new(EspBackend {
        window: RefCell::new(None),
        state: RefCell::new(Some(BoardState {
            display_panel: Some(display_panel),
            cpu_control: Some(peripherals.CPU_CTRL),
            buffer_provider,
            touch,
            i2c,
            _backlight: backlight,
            _int_pin: int_pin,
        })),
    }))
    .expect("backend already initialized");
    peripherals.USB_DEVICE
}

impl EspBackend {
    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let BoardState {
            mut buffer_provider,
            touch,
            i2c,
            _backlight,
            _int_pin,

            cpu_control,

            display_panel,
        } = self.state.borrow_mut().take().expect("event loop already running");

        let mut cpu_control = esp_hal::system::CpuControl::new(cpu_control.unwrap());

        static DISPLAY_STACK: static_cell::ConstStaticCell<esp_hal::system::Stack<16_384>> =
            static_cell::ConstStaticCell::new(esp_hal::system::Stack::new());

        let display_panel = display_panel.unwrap();

        let (touch_producer, touch_consumer) =
            TOUCH_SAMPLES.init(touch_samples::TouchSamples::new()).split();

        let mut touch_receiver = touch_samples::TouchReceiver::new(touch_consumer);

        let touch_worker = touch_worker::TouchWorker::new(i2c, touch, touch_producer);

        let _display_guard = cpu_control
            .start_app_core(DISPLAY_STACK.take(), move || {
                queue().display_with_touch(display_panel, touch_worker)
            })
            .unwrap();

        let mut last_tick = u32::MAX;

        let mut last_touch = None;

        let mut touch_irq = _int_pin;

        loop {
            if touch_irq.is_interrupt_set() {
                touch_irq.clear_interrupt();
                touch_irq.listen(esp_hal::gpio::Event::AnyEdge);
                touch_worker::REQUEST.store(true, Ordering::Release);
            }

            {
                if queue().tick.load(Ordering::Acquire) == last_tick {
                    esp_hal::rom::ets_delay_us(100);
                    continue;
                }
                let (tick, _blank) = queue().phase_snapshot();
                last_tick = tick;
            }

            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                let mut sample_time = None;

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

                let rendered = window.draw_if_needed(|renderer| {
                    buffer_provider.render(renderer);
                });

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
struct DrawBuffer<'a> {
    first_line_frame: bool,
    line_damage: Box<frame_queue::pixel_damage::Plan>,

    frame_buffer: &'a mut [slint::platform::software_renderer::Rgb565BigEndianPixel],
}

type LinePixel = pie_pixel::Pixel;

fn canonical_line(
    pixels: &[LinePixel],
) -> &[slint::platform::software_renderer::Rgb565BigEndianPixel] {
    pie_pixel::as_be(pixels)
}

struct FramebufferLineBuffer<'a> {
    scratch: &'a mut [LinePixel],
    framebuffer: &'a mut [slint::platform::software_renderer::Rgb565BigEndianPixel],
    damage: &'a mut frame_queue::pixel_damage::Plan,
}

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

fn render_full_frame(
    renderer: &slint::platform::software_renderer::SoftwareRenderer,
    pixels: &mut [slint::platform::software_renderer::Rgb565BigEndianPixel],
) -> slint::platform::software_renderer::PhysicalRegion {
    let pixels = pie_pixel::as_mut(pixels);
    renderer.render_into_buffer(&mut native_buffer::FrameBuffer::new(pixels, 320))
}

impl DrawBuffer<'_> {
    fn render(&mut self, renderer: &slint::platform::software_renderer::SoftwareRenderer) {
        #[cfg(feature = "box-display-profile")]
        let started = now_us();

        let region = if !self.first_line_frame {
            self.line_damage.reset();
            #[repr(align(4))]
            struct Scanline([LinePixel; 320]);
            let mut scratch = Scanline([LinePixel::default(); 320]);
            let mut provider = FramebufferLineBuffer {
                scratch: &mut scratch.0,
                framebuffer: self.frame_buffer,

                damage: &mut self.line_damage,
            };
            let region = renderer.render_by_line(&mut provider);
            provider.damage.fit_packet(
                frame_queue::INTERNAL_PACKET_BYTES
                    / core::mem::size_of::<slint::platform::software_renderer::Rgb565BigEndianPixel>(),
            );
            region
        } else {
            render_full_frame(renderer, self.frame_buffer)
        };

        if !self.first_line_frame {
            queue().publish_damage(self.frame_buffer, &self.line_damage);
        } else {
            queue().publish(self.frame_buffer, &region);
            self.first_line_frame = region.iter().next().is_none();
        }
        #[cfg(feature = "box-display-profile")]
        info!("BOX_FRAME,render_and_queue_us={}", now_us() - started);
    }
}
