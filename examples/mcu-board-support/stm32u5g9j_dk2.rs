// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::{Cell, RefCell};
pub use cortex_m_rt::entry;
use defmt_rtt as _;
use slint::platform::{software_renderer, PointerEventButton, WindowEvent};

use embassy_stm32::{
    bind_interrupts,
    gpio::{Level, Output, Speed},
    i2c::I2c,
    ltdc::{
        self, Ltdc, LtdcConfiguration, LtdcLayer, LtdcLayerConfig, PolarityActive, PolarityEdge,
    },
    peripherals, rng,
    time::Hertz,
};
use embassy_stm32::{rcc, Config};

#[cfg(feature = "panic-probe")]
use panic_probe as _;

use embedded_alloc::LlffHeap as Heap;

const HEAP_SIZE: usize = 200 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

bind_interrupts!(struct Irqs {
    LTDC => ltdc::InterruptHandler<peripherals::LTDC>;
    RNG => rng::InterruptHandler<peripherals::RNG>;
});

const DISPLAY_WIDTH: usize = 800;
const DISPLAY_HEIGHT: usize = 480;

/// The Pixel type of the backing store
pub type TargetPixel = software_renderer::Rgb565Pixel;

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

static GLOBAL_RNG: cortex_m::interrupt::Mutex<
    core::cell::RefCell<Option<embassy_stm32::rng::Rng<embassy_stm32::peripherals::RNG>>>,
> = cortex_m::interrupt::Mutex::new(core::cell::RefCell::new(None));

/*
static QUIT_LOOP: cortex_m::interrupt::Mutex<core::cell::Cell<bool>> =
    cortex_m::interrupt::Mutex::new(core::cell::Cell::new(false));
    */

pub fn init() {
    unsafe { ALLOCATOR.init(core::ptr::addr_of_mut!(HEAP) as usize, HEAP_SIZE) }
    slint::platform::set_platform(Box::new(StmBackend::default()))
        .expect("backend already initialized");
}

static mut FB1: [TargetPixel; DISPLAY_WIDTH * DISPLAY_HEIGHT] =
    [software_renderer::Rgb565Pixel(0); DISPLAY_WIDTH * DISPLAY_HEIGHT];

static mut FB2: [TargetPixel; DISPLAY_WIDTH * DISPLAY_HEIGHT] =
    [software_renderer::Rgb565Pixel(0); DISPLAY_WIDTH * DISPLAY_HEIGHT];

struct StmBackendInner {}

struct StmBackend {
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow>>>,
    window_changed: Cell<bool>,
    inner: RefCell<StmBackendInner>,
}

impl Default for StmBackend {
    fn default() -> Self {
        // setup power and clocks for an STM32U5G9J-DK2 run from an external 16 Mhz external oscillator
        let mut config = Config::default();
        config.rcc.hse = Some(rcc::Hse { freq: Hertz(16_000_000), mode: rcc::HseMode::Oscillator });
        config.rcc.pll1 = Some(rcc::Pll {
            source: rcc::PllSource::HSE,
            prediv: rcc::PllPreDiv::DIV1,
            mul: rcc::PllMul::MUL10,
            divp: None,
            divq: None,
            divr: Some(rcc::PllDiv::DIV1),
        });
        config.rcc.sys = rcc::Sysclk::PLL1_R; // 160 Mhz
        config.rcc.pll3 = Some(rcc::Pll {
            source: rcc::PllSource::HSE,
            prediv: rcc::PllPreDiv::DIV4, // PLL_M
            mul: rcc::PllMul::MUL125,     // PLL_N
            divp: None,
            divq: None,
            divr: Some(rcc::PllDiv::DIV20),
        });
        config.rcc.mux.ltdcsel = rcc::mux::Ltdcsel::PLL3_R; // 25 MHz
        let peripherals = embassy_stm32::init(config);

        // enable instruction cache
        embassy_stm32::pac::ICACHE.cr().write(|w| {
            w.set_en(true);
        });

        // enable data cache 1
        // NOTE: be careful of using dcache as some stm32 mcus (especially stm32h7 lines) do not work well with DMA and data cache
        // you may need to disable dcache for specific memory regions (for example FB1 and FB2) or disable dcache altogether

        embassy_stm32::pac::DCACHE1.cr().write(|w| {
            w.set_en(true);
        });

        // enable data cache 2
        embassy_stm32::pac::DCACHE2.cr().write(|w| {
            w.set_en(true);
        });

        // Init RNG

        let rng = embassy_stm32::rng::Rng::new(peripherals.RNG, Irqs);
        cortex_m::interrupt::free(|cs| {
            let _ = GLOBAL_RNG.borrow(cs).replace(Some(rng));
        });

        Self {
            window: RefCell::default(),
            window_changed: Default::default(),
            inner: RefCell::new(StmBackendInner {}),
        }
    }
}

impl slint::platform::Platform for StmBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
            slint::platform::software_renderer::RepaintBufferType::SwappedBuffers,
        );
        window.set_size(
            slint::PhysicalSize::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32)
                .to_logical(window.scale_factor()),
        );
        self.window.replace(Some(window.clone()));
        self.window_changed.set(true);
        Ok(window)
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let mut executor = embassy_executor::Executor::new();
        let static_executor: &'static mut embassy_executor::Executor =
            unsafe { core::mem::transmute(&mut executor) };

        static_executor.run(|spawner| {
            let this = unsafe { core::mem::transmute::<&'_ StmBackend, &'static StmBackend>(self) };
            spawner.must_spawn(main_loop_task(this));
        });
        Ok(())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn slint::platform::EventLoopProxy>> {
        Some(Box::new(DummyLoopProxy {}))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        embassy_time::Instant::now().duration_since(embassy_time::Instant::from_secs(0)).into()
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        use alloc::string::ToString;
        defmt::println!("{=str}", arguments.to_string());
    }
}

impl StmBackend {
    async fn run_loop(&self) {
        let p = unsafe { embassy_stm32::Peripherals::steal() };

        // set up the LTDC peripheral to send data to the LCD screen
        // numbers from STM32U5G9J-DK2.ioc
        const RK050HR18H_HSYNC: u16 = 5; // Horizontal synchronization
        const RK050HR18H_HBP: u16 = 8; // Horizontal back porch
        const RK050HR18H_HFP: u16 = 8; // Horizontal front porch
        const RK050HR18H_VSYNC: u16 = 5; // Vertical synchronization
        const RK050HR18H_VBP: u16 = 8; // Vertical back porch
        const RK050HR18H_VFP: u16 = 8; // Vertical front porch

        // NOTE: all polarities have to be reversed with respect to the STM32U5G9J-DK2 CubeMX parametrization
        let ltdc_config = LtdcConfiguration {
            active_width: DISPLAY_WIDTH as _,
            active_height: DISPLAY_HEIGHT as _,
            h_back_porch: RK050HR18H_HBP,
            h_front_porch: RK050HR18H_HFP,
            v_back_porch: RK050HR18H_VBP,
            v_front_porch: RK050HR18H_VFP,
            h_sync: RK050HR18H_HSYNC,
            v_sync: RK050HR18H_VSYNC,
            h_sync_polarity: PolarityActive::ActiveHigh,
            v_sync_polarity: PolarityActive::ActiveHigh,
            data_enable_polarity: PolarityActive::ActiveHigh,
            pixel_clock_polarity: PolarityEdge::RisingEdge,
        };

        let mut ltdc_de = Output::new(p.PD6, Level::Low, Speed::High);
        let mut ltdc_disp_ctrl = Output::new(p.PE4, Level::Low, Speed::High);
        let mut ltdc_bl_ctrl = Output::new(p.PE6, Level::Low, Speed::High);
        let mut ltdc = Ltdc::new_with_pins(
            p.LTDC, // PERIPHERAL
            Irqs,   // IRQS
            p.PD3,  // CLK
            p.PE0,  // HSYNC
            p.PD13, // VSYNC
            p.PB9,  // B0
            p.PB2,  // B1
            p.PD14, // B2
            p.PD15, // B3
            p.PD0,  // B4
            p.PD1,  // B5
            p.PE7,  // B6
            p.PE8,  // B7
            p.PC8,  // G0
            p.PC9,  // G1
            p.PE9,  // G2
            p.PE10, // G3
            p.PE11, // G4
            p.PE12, // G5
            p.PE13, // G6
            p.PE14, // G7
            p.PC6,  // R0
            p.PC7,  // R1
            p.PE15, // R2
            p.PD8,  // R3
            p.PD9,  // R4
            p.PD10, // R5
            p.PD11, // R6
            p.PD12, // R7
        );
        ltdc.init(&ltdc_config);
        ltdc_de.set_low();
        ltdc_bl_ctrl.set_high();
        ltdc_disp_ctrl.set_high();

        // we only need to draw on one layer for this example (not to be confused with the double buffer)
        let layer_config = LtdcLayerConfig {
            pixel_format: ltdc::PixelFormat::RGB565, // 2 bytes per pixel
            layer: LtdcLayer::Layer1,
            window_x0: 0,
            window_x1: DISPLAY_WIDTH as _,
            window_y0: 0,
            window_y1: DISPLAY_HEIGHT as _,
        };

        ltdc.init_layer(&layer_config, None);

        // used for the touch events
        // NOTE: Async i2c communication returns a Timeout error so we will use blocking i2c until this is fixed
        let mut i2c: I2c<'_, embassy_stm32::mode::Blocking> =
            I2c::new_blocking(p.I2C2, p.PF1, p.PF0, Hertz(100_000), Default::default());

        let touch = gt911::Gt911Blocking::default();
        touch.init(&mut i2c).unwrap();

        let mut scb = cortex_m::Peripherals::take().unwrap().SCB;

        /*
        cortex_m::interrupt::free(|cs| QUIT_LOOP.borrow(cs).set(false));

        let mut ft5336 =
            ft5336::Ft5336::new(&mut inner.touch_i2c, 0x70 >> 1, &mut inner.delay).unwrap();
        ft5336.init(&mut inner.touch_i2c);
        */

        // Safety: The Refcell at the beginning of `run_event_loop` prevents re-entrancy and thus multiple mutable references to FB1/FB2.
        let (fb1, fb2) =
            unsafe { (&mut *core::ptr::addr_of_mut!(FB1), &mut *core::ptr::addr_of_mut!(FB2)) };

        let mut displayed_fb: &mut [TargetPixel] = fb1;
        let mut work_fb: &mut [TargetPixel] = fb2;

        let mut last_touch = None;

        loop {
            slint::platform::update_timers_and_animations();

            if self.window_changed.take() {
                let window = self.window.borrow();
                let window = window.as_ref().unwrap();
                window.set_size(
                    slint::PhysicalSize::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32)
                        .to_logical(window.scale_factor()),
                );
            }

            let maybe_window = (*self.window.borrow()).clone();

            if let Some(window) = maybe_window {
                let mut drawn = false;

                window.draw_if_needed(|renderer| {
                    renderer.render(work_fb, DISPLAY_WIDTH);
                    drawn = true;
                });

                if drawn {
                    scb.clean_dcache_by_slice(work_fb);
                    // Safety: the frame buffer has the right size
                    ltdc.set_buffer(LtdcLayer::Layer1, work_fb.as_ptr() as *const ())
                        .await
                        .unwrap();
                    // Swap the buffer pointer so we will work now on the second buffer
                    core::mem::swap::<&mut [_]>(&mut work_fb, &mut displayed_fb);
                }

                match touch.get_touch(&mut i2c) {
                    Ok(point) => {
                        let button = PointerEventButton::Left;
                        let event = match point {
                            Some(point) => {
                                let position =
                                    slint::PhysicalPosition::new(point.x as i32, point.y as i32)
                                        .to_logical(window.scale_factor());
                                Some(match last_touch.replace(position) {
                                    Some(_) => WindowEvent::PointerMoved { position },
                                    None => WindowEvent::PointerPressed { position, button },
                                })
                            }
                            None => last_touch
                                .take()
                                .map(|position| WindowEvent::PointerReleased { position, button }),
                        };

                        if let Some(event) = event {
                            let is_pointer_release_event =
                                matches!(event, WindowEvent::PointerReleased { .. });
                            window.dispatch_event(event);

                            // removes hover state on widgets
                            if is_pointer_release_event {
                                window.dispatch_event(WindowEvent::PointerExited);
                            }
                        }
                    }
                    Err(gt911::Error::I2C(e)) => {
                        defmt::error!("failed to get touch point: {:?}", e);
                    }
                    Err(_) => {
                        // ignore as these are expected NotReady messages from the touchscreen
                    }
                }
            }

            /*
            if let Some(duration) = slint::platform::duration_until_next_timer_update()
                .and_then(|core_duration| embassy_time::Duration::try_from(core_duration).ok())
            {
                embassy_time::Timer::after(duration).await;
            }
            */

            /*
            if cortex_m::interrupt::free(|cs| QUIT_LOOP.borrow(cs).get()) {
                break;
            }
            */

            // FIXME: cortex_m::asm::wfe();
        }
    }
}

#[embassy_executor::task()]
async fn main_loop_task(backend: &'static StmBackend) {
    backend.run_loop().await;
}

struct DummyLoopProxy;

impl slint::platform::EventLoopProxy for DummyLoopProxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        //cortex_m::interrupt::free(|cs| QUIT_LOOP.borrow(cs).set(true));
        todo!();
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        _event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        unimplemented!()
    }
}

fn rng(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    use rand_core::RngCore;
    cortex_m::interrupt::free(|cs| match GLOBAL_RNG.borrow(cs).borrow_mut().as_mut() {
        Some(rng) => {
            embassy_stm32::rng::Rng::try_fill_bytes(rng, buf).unwrap();
            Ok(())
        }
        None => Err(getrandom::Error::UNSUPPORTED),
    })
}

getrandom::register_custom_getrandom!(rng);
