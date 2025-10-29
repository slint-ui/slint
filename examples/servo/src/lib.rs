// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod adapter;
mod constants;
mod delegate;
mod on_events;
mod rendering_context;
mod servo_util;
mod waker;

#[cfg(not(target_os = "android"))]
mod application_handler;

#[cfg(target_os = "linux")]
mod gl_bindings {
    #![allow(unsafe_op_in_unsafe_fn)]

    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

use slint::ComponentHandle;
use smol::channel;
use std::{cell::RefCell, rc::Rc};

use crate::{
    adapter::{SlintServoAdapter, upgrade_adapter},
    on_events::on_app_callbacks,
    servo_util::{init_servo_webview, spin_servo_event_loop},
};

slint::include_modules!();

#[cfg(not(target_os = "android"))]
use {
    crate::application_handler::ApplicationHandler,
    slint::wgpu_27::{WGPUConfiguration, WGPUSettings, wgpu},
};

pub fn main() {
    let (waker_sender, waker_receiver) = channel::unbounded::<()>();

    let state_placeholder = Rc::new(RefCell::new(None));

    #[cfg(not(target_os = "android"))]
    {
        let application_handler = ApplicationHandler::new(state_placeholder.clone());

        let mut wgpu_settings = WGPUSettings::default();
        wgpu_settings.device_required_features = wgpu::Features::PUSH_CONSTANTS;
        wgpu_settings.device_required_limits.max_push_constant_size =
            constants::MAX_PUSH_CONSTANT_SIZE;

        slint::BackendSelector::new()
        .require_wgpu_27(WGPUConfiguration::Automatic(wgpu_settings))
        .with_winit_custom_application_handler(application_handler)
        .select()
        .expect("Failed to create Slint backend with WGPU based renderer - ensure your system supports WGPU");
    }

    let app = MyApp::new().expect("Failed to create Slint application - check UI resources");

    let app_weak = app.as_weak();

    let adapter = Rc::new(SlintServoAdapter::new(
        app_weak,
        waker_sender.clone(),
        waker_receiver.clone(),
    ));

    let adapter_weak = Rc::downgrade(&adapter);

    #[cfg(not(target_os = "android"))]
    app.window()
        .set_rendering_notifier(move |rendering_state, graphics_api| match rendering_state {
            slint::RenderingState::RenderingSetup => {
                if let slint::GraphicsAPI::WGPU27 { device, queue, .. } = graphics_api {
                    let adpater = upgrade_adapter(&adapter_weak);
                    adpater.set_wgpu_device_queue(device, queue);
                }
            }
            slint::RenderingState::BeforeRendering => {}
            slint::RenderingState::AfterRendering => {}
            slint::RenderingState::RenderingTeardown => {}
            _ => {}
        })
        .expect("Failed to set rendering notifier - WGPU integration may not be available");

    // Update the placeholder with the actual state
    *state_placeholder.borrow_mut() = Some(adapter.clone());

    init_servo_webview(adapter.clone());

    spin_servo_event_loop(adapter.clone());

    on_app_callbacks(adapter.clone());

    app.run()
        .expect("Application failed to run - check for runtime errors");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(android_app: slint::android::AndroidApp) {
    slint::android::init(android_app).unwrap();
    main();
}
