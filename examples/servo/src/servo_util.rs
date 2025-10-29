// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use euclid::{Scale, Size2D};
use slint::ComponentHandle;
use url::Url;
use winit::dpi::PhysicalSize;

use servo::{
    RenderingContext, Servo, ServoBuilder, WebViewBuilder, webrender_api::units::DevicePixel,
};

use crate::{
    WebviewLogic,
    adapter::{SlintServoAdapter, upgrade_adapter},
    delegate::AppDelegate,
    rendering_context::ServoRenderingAdapter,
    waker::Waker,
};

#[cfg(not(target_os = "android"))]
use crate::rendering_context::try_create_gpu_context;

#[cfg(target_os = "android")]
use crate::rendering_context::create_software_context;

pub fn spin_servo_event_loop(state: Rc<SlintServoAdapter>) {
    let state_weak = Rc::downgrade(&state);

    slint::spawn_local({
        async move {
            let state = upgrade_adapter(&state_weak);

            loop {
                let _ = state.waker_reciver().recv().await;
                if let Some(ref servo) = *state.servo.borrow() {
                    servo.spin_event_loop();
                }
            }
        }
    })
    .expect("Failed to spawn servo event loop task");
}

pub fn init_servo_webview(state: Rc<SlintServoAdapter>) {
    let state_weak = Rc::downgrade(&state);

    slint::spawn_local({
        async move {
            let state = upgrade_adapter(&state_weak);

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
                try_create_gpu_context(wgpu_device, wgpu_queue, physical_size).unwrap()
            };

            #[cfg(target_os = "android")]
            let rendering_adapter = create_software_context(physical_size);

            let rendering_context = rendering_adapter.get_rendering_context();

            let servo = intit_servo(state.clone(), rendering_context);

            init_webview(scale_factor, physical_size, state, servo, rendering_adapter);
        }
    })
    .unwrap();
}

fn intit_servo(state: Rc<SlintServoAdapter>, rendering_context: Rc<dyn RenderingContext>) -> Servo {
    let waker = Waker::new(state.waker_sender());

    let event_loop_waker = Box::new(waker);

    ServoBuilder::new(rendering_context)
        .event_loop_waker(event_loop_waker)
        .build()
}

fn init_webview(
    scale_factor: f32,
    physical_size: PhysicalSize<u32>,
    state: Rc<SlintServoAdapter>,
    servo: Servo,
    rendering_adapter: Box<dyn ServoRenderingAdapter>,
) {
    let scale = Scale::new(scale_factor);

    let url = state.app().get_url();

    let url = Url::parse(url.as_str()).expect("Failed to parse url");

    let delegate = Rc::new(AppDelegate::new(state.clone()));

    let webview = WebViewBuilder::new(&servo)
        .url(url)
        .size(physical_size)
        .delegate(delegate)
        .hidpi_scale_factor(scale)
        .build();

    webview.show(true);

    state.set_inner(servo, webview, scale_factor, rendering_adapter);
}
