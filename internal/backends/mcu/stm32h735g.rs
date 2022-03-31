// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

extern crate alloc;

use super::TargetPixel;
pub use cortex_m_rt::entry;
use embedded_display_controller::{DisplayController, DisplayControllerLayer};
use embedded_graphics::prelude::RgbColor;
use embedded_hal::digital::v2::OutputPin;
use hal::gpio::Speed::High;
use hal::ltdc::LtdcLayer1;
use hal::pac;
use hal::prelude::*;
use hal::rcc::rec::OctospiClkSelGetter;
use stm32h7xx_hal as hal;

use defmt_rtt as _; // global logger

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[alloc_error_handler]
fn oom(layout: core::alloc::Layout) -> ! {
    panic!("Out of memory {:?}", layout);
}
use alloc_cortex_m::CortexMHeap;

use crate::{Devices, PhysicalRect, PhysicalSize};

const HEAP_SIZE: usize = 128 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

//const DISPLAY_WIDTH: usize = 320;
//const DISPLAY_HEIGHT: usize = 240;

const DISPLAY_WIDTH: usize = 480;
const DISPLAY_HEIGHT: usize = 272;

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

pub fn init() {
    let mut cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    unsafe { ALLOCATOR.init(&mut HEAP as *const u8 as usize, core::mem::size_of_val(&HEAP)) }

    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.smps().freeze();
    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .sys_ck(320.mhz())
        // there number are just random :-/  Don't know where to find the actual number i need
        .pll3_p_ck(200.mhz())
        .pll3_r_ck(200.mhz())
        .freeze(pwrcfg, &dp.SYSCFG);

    // Octospi from HCLK at 160MHz
    assert_eq!(ccdr.clocks.hclk().0, 160_000_000);
    assert_eq!(
        ccdr.peripheral.OCTOSPI1.get_kernel_clk_mux(),
        hal::rcc::rec::OctospiClkSel::RCC_HCLK3
    );

    cp.SCB.invalidate_icache();
    cp.SCB.enable_icache();
    cp.DWT.enable_cycle_counter();

    let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);
    let gpiof = dp.GPIOF.split(ccdr.peripheral.GPIOF);
    let gpiog = dp.GPIOG.split(ccdr.peripheral.GPIOG);

    // setup OCTOSPI HyperRAM
    let _tracweswo = gpiob.pb3.into_alternate_af0();
    let _ncs = gpiog.pg12.into_alternate_af3().set_speed(High).internal_pull_up(true);
    let _dqs = gpiof.pf12.into_alternate_af9().set_speed(High).internal_pull_up(true);
    let _clk = gpiof.pf4.into_alternate_af9().set_speed(High).internal_pull_up(true);
    let _io0 = gpiof.pf0.into_alternate_af9().set_speed(High).internal_pull_up(true);
    let _io1 = gpiof.pf1.into_alternate_af9().set_speed(High).internal_pull_up(true);
    let _io2 = gpiof.pf2.into_alternate_af9().set_speed(High).internal_pull_up(true);
    let _io3 = gpiof.pf3.into_alternate_af9().set_speed(High).internal_pull_up(true);
    let _io4 = gpiog.pg0.into_alternate_af9().set_speed(High).internal_pull_up(true);
    let _io5 = gpiog.pg1.into_alternate_af9().set_speed(High).internal_pull_up(true);
    let _io6 = gpiog.pg10.into_alternate_af3().set_speed(High).internal_pull_up(true);
    let _io7 = gpiog.pg11.into_alternate_af9().set_speed(High).internal_pull_up(true);

    let hyperram_size = 16 * 1024 * 1024; // 16 MByte
    let config = hal::xspi::HyperbusConfig::new(80.mhz())
        .device_size_bytes(24) // 16 Mbyte
        .refresh_interval(4.us())
        .read_write_recovery(4) // 50ns
        .access_initial_latency(6);

    let hyperram =
        dp.OCTOSPI2.octospi_hyperbus_unchecked(config, &ccdr.clocks, ccdr.peripheral.OCTOSPI2);
    let hyperram_ptr: *mut u32 = hyperram.init();

    i_slint_core::debug_log!("hello");

    let mut led = gpioc.pc2.into_push_pull_output();
    led.set_high().unwrap();
    let mut led2 = gpioc.pc3.into_push_pull_output();
    led2.set_low().unwrap();

    #[link_section = ".frame_buffer"]
    static mut FB1: [TargetPixel; DISPLAY_WIDTH * DISPLAY_HEIGHT] =
        [TargetPixel::BLACK; DISPLAY_WIDTH * DISPLAY_HEIGHT];
    #[link_section = ".frame_buffer"]
    static mut FB2: [TargetPixel; DISPLAY_WIDTH * DISPLAY_HEIGHT] =
        [TargetPixel::BLACK; DISPLAY_WIDTH * DISPLAY_HEIGHT];
    // SAFETY the init function is only called once (as enforced by Peripherals::take)
    let (fb1, fb2) = unsafe { (&mut FB1, &mut FB2) };

    assert!((hyperram_ptr as usize..hyperram_ptr as usize + hyperram_size)
        .contains(&(fb1.as_ptr() as usize)));
    assert!((hyperram_ptr as usize..hyperram_ptr as usize + hyperram_size)
        .contains(&(fb2.as_ptr() as usize)));

    fb1.fill(TargetPixel::BLUE);

    // TODO: do the flip instead
    let mut ltdc = hal::ltdc::Ltdc::new(dp.LTDC, ccdr.peripheral.LTDC, &ccdr.clocks);

    const RK043FN48H_HSYNC: u16 = 41;
    const RK043FN48H_VSYNC: u16 = 10;

    ltdc.init(embedded_display_controller::DisplayConfiguration {
        active_width: DISPLAY_WIDTH as _,
        active_height: DISPLAY_HEIGHT as _,
        h_back_porch: 0,
        h_front_porch: 0,
        v_back_porch: 0,
        v_front_porch: 0,
        h_sync: RK043FN48H_HSYNC - 1,
        v_sync: RK043FN48H_VSYNC - 1,
        h_sync_pol: false,
        v_sync_pol: false,
        not_data_enable_pol: false,
        pixel_clock_pol: false,
    });
    ltdc.listen();
    let mut layer = ltdc.split();

    let mut lcd_disp_ctrl = gpiod.pd10.into_push_pull_output();
    lcd_disp_ctrl.set_high().unwrap();
    let mut lcd_bl_ctrl = gpiog.pg15.into_push_pull_output();
    lcd_bl_ctrl.set_high().unwrap();

    // Safety: the frame buffer has the right size
    unsafe {
        layer.enable(fb1.as_ptr() as *const u16, embedded_display_controller::PixelFormat::RGB565);
        layer.swap_framebuffer(fb1.as_ptr() as *const u16);
    }

    crate::init_with_display(StmDevices { work_fb: fb1 /*displayed_fb: fb2*/, layer });
}

struct StmDevices {
    work_fb: &'static mut [TargetPixel],
    //displayed_fb: &'static mut [TargetPixel],
    layer: LtdcLayer1,
}

impl Devices for StmDevices {
    fn screen_size(&self) -> PhysicalSize {
        PhysicalSize::new(DISPLAY_WIDTH as _, DISPLAY_HEIGHT as _)
    }

    fn fill_region(&mut self, region: PhysicalRect, pixels: &[super::TargetPixel]) {
        let region = region.cast();
        self.work_fb[region.min_y() * DISPLAY_WIDTH + region.min_x()
            ..region.min_y() * DISPLAY_WIDTH + region.max_x()]
            .copy_from_slice(&pixels[region.min_x()..region.max_x()])
    }

    fn debug(&mut self, text: &str) {
        //self.display.debug(text)
    }

    fn read_touch_event(&mut self) -> Option<i_slint_core::input::MouseEvent> {
        None
    }

    fn time(&self) -> core::time::Duration {
        //FIXME
        core::time::Duration::from_micros(0)
    }
}
