// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
pub use cortex_m_rt::entry;
use defmt_rtt as _;
use slint::platform::{software_renderer, PointerEventButton, WindowEvent};

use crate::embassy::{EmbassyBackend, PlatformBackend};
use embassy_stm32::{
    bind_interrupts,
    gpio::{Level, Output, Speed},
    hspi::{ChipSelectHighTime, FIFOThresholdLevel, Hspi, MemorySize, MemoryType, WrapSize},
    i2c::I2c,
    ltdc::{
        self, Ltdc, LtdcConfiguration, LtdcLayer, LtdcLayerConfig, PolarityActive, PolarityEdge,
    },
    peripherals, rng,
    time::Hertz,
};
use embassy_stm32::{rcc, Config};

mod hspi;

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

pub fn init() {
    unsafe { ALLOCATOR.init(core::ptr::addr_of_mut!(HEAP) as usize, HEAP_SIZE) }

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
    hspi::rcc_init(&mut config);
    let p = embassy_stm32::init(config);

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
    let rng = embassy_stm32::rng::Rng::new(p.RNG, Irqs);
    cortex_m::interrupt::free(|cs| {
        let _ = GLOBAL_RNG.borrow(cs).replace(Some(rng));
    });

    let flash_config = embassy_stm32::hspi::Config {
        fifo_threshold: FIFOThresholdLevel::_4Bytes,
        memory_type: MemoryType::Macronix,
        device_size: MemorySize::_1GiB,
        chip_select_high_time: ChipSelectHighTime::_2Cycle,
        free_running_clock: false,
        clock_mode: false,
        wrap_size: WrapSize::None,
        clock_prescaler: 0,
        sample_shifting: false,
        delay_hold_quarter_cycle: false,
        chip_select_boundary: 0,
        delay_block_bypass: false,
        max_transfer: 0,
        refresh: 0,
    };

    let hspi = Hspi::new_octospi(
        p.HSPI1,
        p.PI3,
        p.PH10,
        p.PH11,
        p.PH12,
        p.PH13,
        p.PH14,
        p.PH15,
        p.PI0,
        p.PI1,
        p.PH9,
        p.PI2,
        p.GPDMA1_CH7,
        flash_config,
    );

    let mut flash = embassy_futures::block_on(hspi::OctaDtrFlashMemory::new(hspi));

    embassy_futures::block_on(flash.enable_mm());

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

    // Safety: The Refcell at the beginning of `run_event_loop` prevents re-entrancy and thus multiple mutable references to FB1/FB2.
    let (fb1, fb2) =
        unsafe { (&mut *core::ptr::addr_of_mut!(FB1), &mut *core::ptr::addr_of_mut!(FB2)) };

    let displayed_fb: &mut [TargetPixel] = fb1;
    let work_fb: &mut [TargetPixel] = fb2;

    let scb = cortex_m::Peripherals::take().unwrap().SCB;

    let stm_backend = StmBackendInner {
        _flash: flash,
        touch,
        i2c,
        ltdc,
        _ltdc_display_enable: ltdc_de,
        _ltdc_backlight_control: ltdc_bl_ctrl,
        _ltdc_display_control: ltdc_disp_ctrl,
        displayed_fb,
        work_fb,
        scb,
        last_touch: None,
    };

    let embassy_backend = EmbassyBackend::new(
        stm_backend,
        slint::PhysicalSize { width: DISPLAY_WIDTH as u32, height: DISPLAY_HEIGHT as u32 },
    );

    slint::platform::set_platform(Box::new(embassy_backend)).expect("backend already initialized");
}

static mut FB1: [TargetPixel; DISPLAY_WIDTH * DISPLAY_HEIGHT] =
    [software_renderer::Rgb565Pixel(0); DISPLAY_WIDTH * DISPLAY_HEIGHT];

static mut FB2: [TargetPixel; DISPLAY_WIDTH * DISPLAY_HEIGHT] =
    [software_renderer::Rgb565Pixel(0); DISPLAY_WIDTH * DISPLAY_HEIGHT];

struct StmBackendInner {
    _flash: hspi::OctaDtrFlashMemory<'static, embassy_stm32::peripherals::HSPI1>,
    touch: gt911::Gt911Blocking<I2c<'static, embassy_stm32::mode::Blocking>>,
    i2c: embassy_stm32::i2c::I2c<'static, embassy_stm32::mode::Blocking>,
    ltdc: embassy_stm32::ltdc::Ltdc<'static, embassy_stm32::peripherals::LTDC>,
    _ltdc_display_enable: embassy_stm32::gpio::Output<'static>,
    _ltdc_backlight_control: embassy_stm32::gpio::Output<'static>,
    _ltdc_display_control: embassy_stm32::gpio::Output<'static>,
    displayed_fb: &'static mut [TargetPixel],
    work_fb: &'static mut [TargetPixel],
    scb: cortex_m::peripheral::SCB,
    last_touch: Option<slint::LogicalPosition>,
}

impl PlatformBackend for StmBackendInner {
    async fn dispatch_events(&mut self, window: &slint::Window) {
        match self.touch.get_touch(&mut self.i2c) {
            Ok(point) => {
                let button = PointerEventButton::Left;
                let event = match point {
                    Some(point) => {
                        let position = slint::PhysicalPosition::new(point.x as i32, point.y as i32)
                            .to_logical(window.scale_factor());
                        Some(match self.last_touch.replace(position) {
                            Some(_) => WindowEvent::PointerMoved { position },
                            None => WindowEvent::PointerPressed { position, button },
                        })
                    }
                    None => self
                        .last_touch
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
    async fn render(&mut self, renderer: &slint::platform::software_renderer::SoftwareRenderer) {
        renderer.render(self.work_fb, DISPLAY_WIDTH);

        self.scb.clean_dcache_by_slice(self.work_fb);
        // Safety: the frame buffer has the right size
        self.ltdc.set_buffer(LtdcLayer::Layer1, self.work_fb.as_ptr() as *const ()).await.unwrap();
        // Swap the buffer pointer so we will work now on the second buffer
        core::mem::swap::<&mut [_]>(&mut self.work_fb, &mut self.displayed_fb);
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
