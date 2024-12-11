// A demo for stm32u5g9j-dk2
// The application renders a simple Slint screen to the display and the user can interact with it
// by toggling the green led on and off as well as pushing the blue button on the dk which should
// turn the grey circle next to "Hardware User Button" blue.
// The hello world button demonstrates animations. More details in the readme.

#![no_std]
#![no_main]
#![macro_use]
#![allow(static_mut_refs)]

extern crate alloc;

use alloc::{boxed::Box, rc::Rc};
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_stm32::{
    bind_interrupts,
    exti::ExtiInput,
    gpio::{Level, Output, Pull, Speed},
    i2c::{self, I2c},
    ltdc::{
        self, Ltdc, LtdcConfiguration, LtdcLayer, LtdcLayerConfig, PolarityActive, PolarityEdge,
    },
    mode::{self},
    peripherals,
    time::Hertz,
};
use embassy_time::{Duration, Timer};
use gt911::Gt911;
use mcu_embassy::{
    controller::{self, Action, Controller},
    mcu::{double_buffer::DoubleBuffer, hardware::HardwareMcu, rcc_setup, ALLOCATOR},
    slint_backend::{StmBackend, TargetPixelType, DISPLAY_HEIGHT, DISPLAY_WIDTH},
};
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel},
        PointerEventButton, WindowEvent,
    },
    ComponentHandle,
};
use slint_generated::MainWindow;
use {defmt_rtt as _, panic_probe as _};

const MY_TASK_POOL_SIZE: usize = 2;
const HEAP_SIZE: usize = 200 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

// the following two display buffers consume 1536000 bytes that just about fits into the ram found on the mcu
static mut FB1: [TargetPixelType; DISPLAY_WIDTH * DISPLAY_HEIGHT] =
    [Rgb565Pixel(0); DISPLAY_WIDTH * DISPLAY_HEIGHT];
static mut FB2: [TargetPixelType; DISPLAY_WIDTH * DISPLAY_HEIGHT] =
    [Rgb565Pixel(0); DISPLAY_WIDTH * DISPLAY_HEIGHT];

bind_interrupts!(struct Irqs {
    LTDC => ltdc::InterruptHandler<peripherals::LTDC>;
    I2C2_EV => i2c::EventInterruptHandler<peripherals::I2C2>;
    I2C2_ER => i2c::ErrorInterruptHandler<peripherals::I2C2>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = rcc_setup::stm32u5g9zj_init();

    // setup an allocator
    unsafe { ALLOCATOR.init(&mut HEAP as *const u8 as usize, core::mem::size_of_val(&HEAP)) }

    // enable instruction cache
    embassy_stm32::pac::ICACHE.cr().write(|w| {
        w.set_en(true);
    });

    // enable data cache 1
    embassy_stm32::pac::DCACHE1.cr().write(|w| {
        w.set_en(true);
    });

    // enable data cache 2
    embassy_stm32::pac::DCACHE2.cr().write(|w| {
        w.set_en(true);
    });

    // used for the touch events
    let i2c = I2c::new(
        p.I2C2,
        p.PF1,
        p.PF0,
        Irqs,
        p.GPDMA1_CH0,
        p.GPDMA1_CH1,
        Hertz(100_000),
        Default::default(),
    );

    // TASK: blink the red led on another task
    let red_led = Output::new(p.PD2, Level::High, Speed::Low);
    unwrap!(spawner.spawn(led_task(red_led)));

    // TASK: wait for hardware user button press
    let user_btn = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);
    unwrap!(spawner.spawn(user_btn_task(user_btn)));

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

    info!("init ltdc");
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
    info!("enable bottom layer");
    let layer_config = LtdcLayerConfig {
        pixel_format: ltdc::PixelFormat::RGB565, // 2 bytes per pixel
        layer: LtdcLayer::Layer1,
        window_x0: 0,
        window_x1: DISPLAY_WIDTH as _,
        window_y0: 0,
        window_y1: DISPLAY_HEIGHT as _,
    };

    // enable the bottom layer
    ltdc.init_layer(&layer_config, None);

    // Safety: the DoubleBuffer controls access to the statically allocated frame buffers
    // and it is the only thing that mutates their content
    let double_buffer =
        DoubleBuffer::new(unsafe { FB1.as_mut() }, unsafe { FB2.as_mut() }, layer_config);

    // create a slint window and register it with slint
    let window = MinimalSoftwareWindow::new(RepaintBufferType::SwappedBuffers);
    window.set_size(slint::PhysicalSize::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32));
    let backend = Box::new(StmBackend::new(window.clone()));
    slint::platform::set_platform(backend).expect("backend already initialized");
    info!("slint gui setup complete");

    // TASK: run the gui render loop
    unwrap!(spawner.spawn(render_loop(window, double_buffer, ltdc, i2c)));

    let main_window = MainWindow::new().unwrap();
    main_window.show().expect("unable to show main window");

    let green_led = Output::new(p.PD4, Level::High, Speed::Low);
    let hardware = HardwareMcu { green_led };

    // run the controller event loop
    let mut controller = Controller::new(&main_window, hardware);
    controller.run().await;
}

#[embassy_executor::task(pool_size = MY_TASK_POOL_SIZE)]
async fn led_task(mut led: Output<'static>) {
    loop {
        // on
        led.set_low();
        Timer::after(Duration::from_millis(50)).await;

        // off
        led.set_high();
        Timer::after(Duration::from_millis(450)).await;
    }
}

// low latency button press with debounce and toggle state recovery (for data races)
#[embassy_executor::task(pool_size = MY_TASK_POOL_SIZE)]
async fn user_btn_task(mut user_btn: ExtiInput<'static>) {
    let mut is_high = false;
    info!("Press the USER button...");

    loop {
        let any_edge = user_btn.wait_for_any_edge();
        let timeout = Timer::after(Duration::from_millis(1000));

        // the timeout is here in case of a data race between the last button check
        // and beginning the wait for an edge change
        match select(any_edge, timeout).await {
            Either::First(_) => {}
            Either::Second(_) => {}
        };

        if user_btn.is_high() != is_high {
            is_high = !is_high;
            info!("Button is pressed: {}", is_high);
            controller::send_action(Action::HardwareUserBtnPressed(is_high));

            // debounce
            Timer::after(Duration::from_millis(50)).await;
        }

        // check button state again as the button may have been
        // released (and remained released) within the debounce period
        if user_btn.is_high() != is_high {
            is_high = !is_high;
            info!("Button is pressed: {}", is_high);
            controller::send_action(Action::HardwareUserBtnPressed(is_high));
        }
    }
}

#[embassy_executor::task()]
pub async fn render_loop(
    window: Rc<MinimalSoftwareWindow>,
    mut double_buffer: DoubleBuffer,
    mut ltdc: Ltdc<'static, peripherals::LTDC>,
    mut i2c: I2c<'static, mode::Async>,
) {
    let mut last_touch: Option<slint::LogicalPosition> = None;
    let touch = Gt911::default();
    touch.init(&mut i2c).await.unwrap();

    loop {
        slint::platform::update_timers_and_animations();

        // process touchscreen events
        process_touch(&touch, &mut i2c, &mut last_touch, window.clone()).await;

        // blocking render
        let is_dirty = window.draw_if_needed(|renderer| {
            let buffer = double_buffer.current();
            renderer.render(buffer, DISPLAY_WIDTH);
        });

        if is_dirty {
            // async transfer of frame buffer to lcd
            double_buffer.swap(&mut ltdc).await.unwrap();
        } else {
            Timer::after(Duration::from_millis(10)).await;
        }
    }
}

async fn process_touch(
    touch: &Gt911<I2c<'static, mode::Async>>,
    i2c: &mut I2c<'static, mode::Async>,
    last_touch: &mut Option<slint::LogicalPosition>,
    window: Rc<MinimalSoftwareWindow>,
) {
    // process touchscreen touch events
    if let Ok(point) = touch.get_touch(i2c).await {
        let button = PointerEventButton::Left;
        let event = match point {
            Some(point) => {
                let position = slint::PhysicalPosition::new(point.x as i32, point.y as i32)
                    .to_logical(window.scale_factor());
                Some(match last_touch.replace(position) {
                    Some(_) => WindowEvent::PointerMoved { position },
                    None => WindowEvent::PointerPressed { position, button },
                })
            }
            None => {
                last_touch.take().map(|position| WindowEvent::PointerReleased { position, button })
            }
        };

        if let Some(event) = event {
            let is_pointer_release_event = matches!(event, WindowEvent::PointerReleased { .. });
            window.dispatch_event(event);

            // removes hover state on widgets
            if is_pointer_release_event {
                window.dispatch_event(WindowEvent::PointerExited);
            }
        }
    }
}
