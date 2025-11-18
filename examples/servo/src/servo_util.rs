// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use smol::channel;
use url::Url;
use winit::dpi::PhysicalSize;

use euclid::{Scale, Size2D};

use i_slint_core::items::ColorScheme;
use slint::{ComponentHandle, SharedString};

use servo::{Servo, ServoBuilder, Theme, WebViewBuilder, webrender_api::units::DevicePixel};

use crate::{
    MyApp, Palette, WebviewLogic,
    adapter::{SlintServoAdapter, upgrade_adapter},
    delegate::AppDelegate,
    on_events::on_app_callbacks,
    rendering_context::ServoRenderingAdapter,
    waker::Waker,
};

pub fn init_servo(
    app_weak: slint::Weak<MyApp>,
    initial_url: SharedString,
    #[cfg(not(target_os = "android"))] device: slint::wgpu_27::wgpu::Device,
    #[cfg(not(target_os = "android"))] queue: slint::wgpu_27::wgpu::Queue,
) -> Rc<SlintServoAdapter> {
    let (waker_sender, waker_receiver) = channel::unbounded::<()>();

    #[cfg(not(target_os = "android"))]
    let adapter = Rc::new(SlintServoAdapter::new(
        app_weak,
        waker_sender.clone(),
        waker_receiver.clone(),
        device,
        queue,
    ));

    #[cfg(target_os = "android")]
    let adapter =
        Rc::new(SlintServoAdapter::new(app_weak, waker_sender.clone(), waker_receiver.clone()));

    let state_weak = Rc::downgrade(&adapter);

    slint::spawn_local({
        async move {
            let state = upgrade_adapter(&state_weak);

            let (rendering_adapter, physical_size, scale_factor) =
                init_rendering_adpater(state.clone());

            let servo = intit_servo_builder(state.clone(), rendering_adapter.clone());

            init_webview(scale_factor, physical_size, initial_url, state, servo, rendering_adapter);
        }
    })
    .unwrap();

    spin_servo_event_loop(adapter.clone());

    on_app_callbacks(adapter.clone());

    adapter
}

fn init_rendering_adpater(
    state: Rc<SlintServoAdapter>,
) -> (Rc<Box<dyn ServoRenderingAdapter>>, PhysicalSize<u32>, f32) {
    let app = state.app();

    let scale_factor = app.window().scale_factor() as f32;

    let width = app.global::<WebviewLogic>().get_viewport_width();
    let height = app.global::<WebviewLogic>().get_viewport_height();

    let size: Size2D<f32, DevicePixel> = Size2D::new(width, height) * scale_factor;

    let physical_size = PhysicalSize::new(size.width as u32, size.height as u32);

    #[cfg(not(target_os = "android"))]
    let rendering_adapter = {
        let wgpu_device = state.wgpu_device();
        let wgpu_queue = state.wgpu_queue();
        crate::rendering_context::try_create_gpu_context(wgpu_device, wgpu_queue, physical_size)
            .unwrap()
    };

    #[cfg(target_os = "android")]
    let rendering_adapter = crate::rendering_context::create_software_context(physical_size);

    let rendering_adapter_rc = Rc::new(rendering_adapter);

    (rendering_adapter_rc, physical_size, scale_factor)
}

fn intit_servo_builder(
    adapter: Rc<SlintServoAdapter>,
    rendering_adapter: Rc<Box<dyn ServoRenderingAdapter>>,
) -> Servo {
    let waker = Waker::new(adapter.waker_sender());

    let event_loop_waker = Box::new(waker);

    let rendering_context = rendering_adapter.get_rendering_context();

    ServoBuilder::new(rendering_context).event_loop_waker(event_loop_waker).build()
}

fn init_webview(
    scale_factor: f32,
    physical_size: PhysicalSize<u32>,
    initial_url: SharedString,
    adapter: Rc<SlintServoAdapter>,
    servo: Servo,
    rendering_adapter: Rc<Box<dyn ServoRenderingAdapter>>,
) {
    let scale = Scale::new(scale_factor);

    let app = adapter.app();

    app.global::<WebviewLogic>().set_current_url(initial_url.clone());

    let url = Url::parse(&initial_url).expect("Failed to parse url");

    let delegate = Rc::new(AppDelegate::new(adapter.clone()));

    let webview = WebViewBuilder::new(&servo)
        .url(url)
        .size(physical_size)
        .delegate(delegate)
        .hidpi_scale_factor(scale)
        .build();

    webview.show(true);

    let color_scheme = app.global::<Palette>().get_color_scheme();

    let theme = if color_scheme == ColorScheme::Dark { Theme::Dark } else { Theme::Light };

    webview.notify_theme_change(theme);

    // Extract the Box from Rc - this requires the Rc to have a strong count of 1
    let rendering_adapter_box = Rc::try_unwrap(rendering_adapter)
        .unwrap_or_else(|_| panic!("Rendering adapter has multiple references"));

    adapter.set_inner(servo, webview, scale_factor, rendering_adapter_box);
}

pub fn spin_servo_event_loop(state: Rc<SlintServoAdapter>) {
    let state_weak = Rc::downgrade(&state);

    slint::spawn_local({
        async move {
            loop {
                let state = match state_weak.upgrade() {
                    Some(s) => s,
                    None => break,
                };

                let _ = state.waker_reciver().recv().await;
                if let Some(ref servo) = *state.servo.borrow() {
                    servo.spin_event_loop();
                }
            }
        }
    })
    .expect("Failed to spawn servo event loop task");
}
