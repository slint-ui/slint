// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
pub use cortex_m_rt::entry;
use defmt_rtt as _;
use embedded_display_controller::{DisplayController, DisplayControllerLayer};
use hal::delay::Delay;
use hal::gpio::Speed::High;
use hal::pac;
use hal::prelude::*;
use slint::platform::software_renderer;
use stm32h7xx_hal as hal; // global logger

#[cfg(feature = "panic-probe")]
use panic_probe as _;

use embedded_alloc::LlffHeap as Heap;

const HEAP_SIZE: usize = 200 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

const DISPLAY_WIDTH: usize = 480;
const DISPLAY_HEIGHT: usize = 272;

/// The Pixel type of the backing store
pub type TargetPixel = software_renderer::Rgb565Pixel;

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

static RNG: cortex_m::interrupt::Mutex<core::cell::RefCell<Option<hal::rng::Rng>>> =
    cortex_m::interrupt::Mutex::new(core::cell::RefCell::new(None));

pub fn init() {
    unsafe { ALLOCATOR.init(core::ptr::addr_of_mut!(HEAP) as usize, HEAP_SIZE) }
    slint::platform::set_platform(Box::new(StmBackend::default()))
        .expect("backend already initialized");
}

#[unsafe(link_section = ".frame_buffer")]
static mut FB1: [TargetPixel; DISPLAY_WIDTH * DISPLAY_HEIGHT] =
    [software_renderer::Rgb565Pixel(0); DISPLAY_WIDTH * DISPLAY_HEIGHT];
#[unsafe(link_section = ".frame_buffer")]
static mut FB2: [TargetPixel; DISPLAY_WIDTH * DISPLAY_HEIGHT] =
    [software_renderer::Rgb565Pixel(0); DISPLAY_WIDTH * DISPLAY_HEIGHT];

struct StmBackendInner {
    scb: cortex_m::peripheral::SCB,
    delay: stm32h7xx_hal::delay::Delay,
    layer: stm32h7xx_hal::ltdc::LtdcLayer1,
    touch_i2c: stm32h7xx_hal::i2c::I2c<stm32h7xx_hal::device::I2C4>,
}

struct StmBackend {
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow>>>,
    timer: hal::timer::Timer<pac::TIM2>,
    inner: RefCell<StmBackendInner>,
}

impl Default for StmBackend {
    fn default() -> Self {
        let mut cp = cortex_m::Peripherals::take().unwrap();
        let dp = pac::Peripherals::take().unwrap();

        let pwr = dp.PWR.constrain();
        let pwrcfg = pwr.smps().freeze();
        let rcc = dp.RCC.constrain();
        let ccdr = rcc
            .sys_ck(400.MHz())
            // numbers adapted from Drivers/BSP/STM32H735G-DK/stm32h735g_discovery_ospi.c
            // MX_OSPI_ClockConfig
            .pll2_p_ck(400.MHz() / 5)
            .pll2_q_ck(400.MHz() / 2)
            .pll2_r_ck(400.MHz() / 2)
            // numbers adapted from Drivers/BSP/STM32H735G-DK/stm32h735g_discovery_lcd.c
            // MX_LTDC_ClockConfig
            .pll3_p_ck(800.MHz() / 2)
            .pll3_q_ck(800.MHz() / 2)
            .pll3_r_ck(800.MHz() / 83)
            .freeze(pwrcfg, &dp.SYSCFG);

        assert_eq!(ccdr.clocks.hclk(), 200.MHz::<1, 1>());

        let mut delay = Delay::new(cp.SYST, ccdr.clocks);

        cp.SCB.invalidate_icache();
        cp.SCB.enable_icache();
        cp.SCB.enable_dcache(&mut cp.CPUID);
        cp.DWT.enable_cycle_counter();

        let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
        let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
        let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
        let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);
        let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);
        let gpiof = dp.GPIOF.split(ccdr.peripheral.GPIOF);
        let gpiog = dp.GPIOG.split(ccdr.peripheral.GPIOG);
        let gpioh = dp.GPIOH.split(ccdr.peripheral.GPIOH);

        // setup OCTOSPI HyperRAM
        let _tracweswo = gpiob.pb3.into_alternate::<0>();
        let _ncs = gpiog.pg12.into_alternate::<3>().speed(High).internal_pull_up(true);
        let _dqs = gpiof.pf12.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _clk = gpiof.pf4.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io0 = gpiof.pf0.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io1 = gpiof.pf1.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io2 = gpiof.pf2.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io3 = gpiof.pf3.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io4 = gpiog.pg0.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io5 = gpiog.pg1.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io6 = gpiog.pg10.into_alternate::<3>().speed(High).internal_pull_up(true);
        let _io7 = gpiog.pg11.into_alternate::<9>().speed(High).internal_pull_up(true);

        let hyperram_size = 16 * 1024 * 1024; // 16 MByte
        let config = hal::xspi::HyperbusConfig::new(80.MHz())
            .device_size_bytes(24) // 16 Mbyte
            .refresh_interval(4.micros())
            .read_write_recovery(4) // 50ns
            .access_initial_latency(6);

        let hyperram =
            dp.OCTOSPI2.octospi_hyperbus_unchecked(config, &ccdr.clocks, ccdr.peripheral.OCTOSPI2);
        let hyperram_ptr: *mut u32 = hyperram.init();

        let _ncs = gpiog.pg6.into_alternate::<10>().speed(High).internal_pull_up(true);
        let _clk = gpiof.pf10.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _dqs = gpiob.pb2.into_alternate::<10>().speed(High).internal_pull_up(true);
        let _io0 = gpiod.pd11.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io1 = gpiod.pd12.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io2 = gpioe.pe2.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io3 = gpiod.pd13.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io4 = gpiod.pd4.into_alternate::<10>().speed(High).internal_pull_up(true);
        let _io5 = gpiod.pd5.into_alternate::<10>().speed(High).internal_pull_up(true);
        let _io6 = gpiog.pg9.into_alternate::<9>().speed(High).internal_pull_up(true);
        let _io7 = gpiod.pd7.into_alternate::<10>().speed(High).internal_pull_up(true);

        use stm32h7xx_hal::xspi::*;
        use OctospiWord as XW;

        let mut octospi =
            dp.OCTOSPI1.octospi_unchecked(12.MHz(), &ccdr.clocks, ccdr.peripheral.OCTOSPI1);

        // Switch Macronix MX25LM51245GXDI00 to SDR OPI
        // Set WREN bit
        octospi.write_extended(XW::U8(0x06), XW::None, XW::None, &[]).unwrap();
        // Write Configuration Register 2
        octospi.write_extended(XW::U8(0x72), XW::U32(0), XW::None, &[1]).unwrap();
        // Change bus mode
        octospi.configure_mode(OctospiMode::EightBit).unwrap();

        const MX25LM51245G_OCTA_READ_CFG_REG2_CMD: u16 = 0x718E;
        const MX25LM51245G_CR2_REG1_ADDR: u32 = 0x00000000;
        const MX25LM51245G_OCTA_READ_CMD: u16 = 0xEC13;

        // check the config register
        let mut read: [u8; 1] = [0];
        octospi
            .read_extended(
                XW::U16(MX25LM51245G_OCTA_READ_CFG_REG2_CMD),
                XW::U32(MX25LM51245G_CR2_REG1_ADDR),
                XW::None,
                5,
                &mut read,
            )
            .unwrap();
        assert_eq!(read[0], 1);

        extern "C" {
            static mut __s_slint_assets: u8;
            static __e_slint_assets: u8;
            static __si_slint_assets: u8;
        }

        unsafe {
            let asset_mem_slice = core::slice::from_raw_parts_mut(
                core::ptr::addr_of_mut!(__s_slint_assets),
                core::ptr::addr_of!(__e_slint_assets) as usize
                    - core::ptr::addr_of!(__s_slint_assets) as usize,
            );
            let mut asset_flash_addr =
                core::ptr::addr_of!(__si_slint_assets) as usize - 0x9000_0000;
            for chunk in asset_mem_slice.chunks_mut(32) {
                octospi
                    .read_extended(
                        XW::U16(MX25LM51245G_OCTA_READ_CMD),
                        XW::U32(asset_flash_addr as u32),
                        XW::None,
                        20,
                        chunk,
                    )
                    .unwrap();
                asset_flash_addr += chunk.len();
            }
        }

        /*
        let mut led_red = gpioc.pc2.into_push_pull_output();
        led_red.set_low(); // low mean "on"
        let mut led_green = gpioc.pc3.into_push_pull_output();
        led_green.set_low();
        */

        #[allow(unused_unsafe)] //(unsafe required for Rust <= 1.81)
        let (fb1, fb2) = unsafe { (core::ptr::addr_of!(FB1), core::ptr::addr_of!(FB2)) };
        assert!((hyperram_ptr as usize..hyperram_ptr as usize + hyperram_size)
            .contains(&(fb1 as usize)));
        assert!((hyperram_ptr as usize..hyperram_ptr as usize + hyperram_size)
            .contains(&(fb2 as usize)));

        // setup LTDC  (LTDC_MspInit)
        let _p = gpioa.pa3.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioa.pa4.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioa.pa6.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpiob.pb0.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpiob.pb1.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpiob.pb8.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpiob.pb9.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioc.pc6.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioc.pc7.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpiod.pd0.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpiod.pd3.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpiod.pd6.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioe.pe0.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioe.pe1.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioe.pe11.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioe.pe12.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioe.pe15.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpiog.pg7.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpiog.pg14.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioh.ph3.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioh.ph8.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioh.ph9.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioh.ph10.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioh.ph11.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioh.ph15.into_alternate::<14>().speed(High).internal_pull_up(true);
        let _p = gpioa.pa8.into_alternate::<13>().speed(High).internal_pull_up(true);
        let _p = gpioh.ph4.into_alternate::<9>().speed(High).internal_pull_up(true);

        let mut lcd_disp_en = gpioe.pe13.into_push_pull_output();
        let mut lcd_disp_ctrl = gpiod.pd10.into_push_pull_output();
        let mut lcd_bl_ctrl = gpiog.pg15.into_push_pull_output();

        delay.delay_ms(40u8);
        // End LTDC_MspInit

        let mut ltdc = hal::ltdc::Ltdc::new(dp.LTDC, ccdr.peripheral.LTDC, &ccdr.clocks);

        const RK043FN48H_HSYNC: u16 = 41; /* Horizontal synchronization */
        const RK043FN48H_HBP: u16 = 13; /* Horizontal back porch      */
        const RK043FN48H_HFP: u16 = 32; /* Horizontal front porch     */
        const RK043FN48H_VSYNC: u16 = 10; /* Vertical synchronization   */
        const RK043FN48H_VBP: u16 = 2; /* Vertical back porch        */
        const RK043FN48H_VFP: u16 = 2; /* Vertical front porch       */

        ltdc.init(embedded_display_controller::DisplayConfiguration {
            active_width: DISPLAY_WIDTH as _,
            active_height: DISPLAY_HEIGHT as _,
            h_back_porch: RK043FN48H_HBP - 11, // -11 from MX_LTDC_Init
            h_front_porch: RK043FN48H_HFP,
            v_back_porch: RK043FN48H_VBP,
            v_front_porch: RK043FN48H_VFP,
            h_sync: RK043FN48H_HSYNC,
            v_sync: RK043FN48H_VSYNC,
            h_sync_pol: false,
            v_sync_pol: false,
            not_data_enable_pol: false,
            pixel_clock_pol: false,
        });
        let mut layer = ltdc.split();

        // Safety: the frame buffer has the right size
        unsafe {
            layer.enable(fb1 as *const u8, embedded_display_controller::PixelFormat::RGB565);
        }

        lcd_disp_en.set_low();
        lcd_disp_ctrl.set_high();
        lcd_bl_ctrl.set_high();

        // Init Timer
        let mut timer = dp.TIM2.tick_timer(10000.Hz(), ccdr.peripheral.TIM2, &ccdr.clocks);
        timer.listen(hal::timer::Event::TimeOut);

        // Init RNG
        let rng = dp.RNG.constrain(ccdr.peripheral.RNG, &ccdr.clocks);
        cortex_m::interrupt::free(|cs| {
            let _ = RNG.borrow(cs).replace(Some(rng));
        });

        // Init Touch screen
        let scl =
            gpiof.pf14.into_alternate::<4>().set_open_drain().speed(High).internal_pull_up(true);
        let sda =
            gpiof.pf15.into_alternate::<4>().set_open_drain().speed(High).internal_pull_up(true);
        let touch_i2c = dp.I2C4.i2c((scl, sda), 100u32.kHz(), ccdr.peripheral.I2C4, &ccdr.clocks);

        Self {
            window: RefCell::default(),
            timer,
            inner: RefCell::new(StmBackendInner { scb: cp.SCB, delay, layer, touch_i2c }),
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
        self.window.replace(Some(window.clone()));
        Ok(window)
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let inner = &mut *self.inner.borrow_mut();

        let mut ft5336 =
            ft5336::Ft5336::new(&mut inner.touch_i2c, 0x70 >> 1, &mut inner.delay).unwrap();
        ft5336.init(&mut inner.touch_i2c);

        // Safety: The Refcell at the beginning of `run_event_loop` prevents re-entrancy and thus multiple mutable references to FB1/FB2.
        let (fb1, fb2) =
            unsafe { (&mut *core::ptr::addr_of_mut!(FB1), &mut *core::ptr::addr_of_mut!(FB2)) };

        let mut displayed_fb: &mut [TargetPixel] = fb1;
        let mut work_fb: &mut [TargetPixel] = fb2;

        let mut last_touch = None;
        self.window
            .borrow()
            .as_ref()
            .unwrap()
            .set_size(slint::PhysicalSize::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32));
        loop {
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                window.draw_if_needed(|renderer| {
                    while inner.layer.is_swap_pending() {}
                    renderer.render(work_fb, DISPLAY_WIDTH);
                    inner.scb.clean_dcache_by_slice(work_fb);
                    // Safety: the frame buffer has the right size
                    unsafe { inner.layer.swap_framebuffer(work_fb.as_ptr() as *const u8) };
                    // Swap the buffer pointer so we will work now on the second buffer
                    core::mem::swap::<&mut [_]>(&mut work_fb, &mut displayed_fb);
                });

                // handle touch event
                let touch = ft5336.detect_touch(&mut inner.touch_i2c).unwrap();
                let button = slint::platform::PointerEventButton::Left;
                let event = if touch > 0 {
                    let state = ft5336.get_touch(&mut inner.touch_i2c, 1).unwrap();
                    let position = slint::PhysicalPosition::new(state.y as i32, state.x as i32)
                        .to_logical(window.scale_factor());
                    Some(match last_touch.replace(position) {
                        Some(_) => slint::platform::WindowEvent::PointerMoved { position },
                        None => slint::platform::WindowEvent::PointerPressed { position, button },
                    })
                } else {
                    last_touch.take().map(|position| {
                        slint::platform::WindowEvent::PointerReleased { position, button }
                    })
                };

                if let Some(event) = event {
                    let is_pointer_release_event =
                        matches!(event, slint::platform::WindowEvent::PointerReleased { .. });

                    window.try_dispatch_event(event)?;

                    // removes hover state on widgets
                    if is_pointer_release_event {
                        window.try_dispatch_event(slint::platform::WindowEvent::PointerExited)?;
                    }
                }
            }

            // FIXME: cortex_m::asm::wfe();
        }
    }

    fn duration_since_start(&self) -> core::time::Duration {
        // FIXME! the timer can overflow
        let val = self.timer.counter() / 10;
        core::time::Duration::from_millis(val.into())
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        use alloc::string::ToString;
        defmt::println!("{=str}", arguments.to_string());
    }
}

fn rng(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    cortex_m::interrupt::free(|cs| match RNG.borrow(cs).borrow_mut().as_mut() {
        Some(rng) => rng.fill(buf).map_err(|e| match e {
            stm32h7xx_hal::rng::ErrorKind::ClockError => getrandom::Error::UNSUPPORTED,
            stm32h7xx_hal::rng::ErrorKind::SeedError => getrandom::Error::UNSUPPORTED,
        }),
        None => Err(getrandom::Error::UNSUPPORTED),
    })
}

getrandom::register_custom_getrandom!(rng);
