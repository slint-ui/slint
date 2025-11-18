// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod adapter;
mod delegate;
mod on_events;
mod rendering_context;
mod servo_util;
mod waker;

#[cfg(target_os = "linux")]
mod gl_bindings {
    #![allow(unsafe_op_in_unsafe_fn)]

    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

use slint::ComponentHandle;
use smol::channel;
use std::rc::Rc;

use crate::{
    adapter::SlintServoAdapter,
    on_events::on_app_callbacks,
    servo_util::{init_servo_webview, spin_servo_event_loop},
};

slint::include_modules!();

pub fn main() {
    let (waker_sender, waker_receiver) = channel::unbounded::<()>();

    #[cfg(not(target_os = "android"))]
    let (device, queue) = {
        use slint::wgpu_27::{WGPUConfiguration, wgpu};

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let adapter = smol::block_on(async {
            instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("Failed to find an appropriate WGPU adapter")
        });

        let (device, queue) = smol::block_on(async {
            adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .expect("Failed to create WGPU device")
        });

        slint::BackendSelector::new()
        .require_wgpu_27(WGPUConfiguration::Manual { 
            instance, 
            adapter, 
            device: device.clone(), 
            queue: queue.clone() 
        })
        .select()
        .expect("Failed to create Slint backend with WGPU based renderer - ensure your system supports WGPU");
        
        (device, queue)
    };

    let app = MyApp::new().expect("Failed to create Slint application - check UI resources");

    let app_weak = app.as_weak();

    #[cfg(not(target_os = "android"))]
    let adapter = Rc::new(SlintServoAdapter::new(
        app_weak,
        waker_sender.clone(),
        waker_receiver.clone(),
        device,
        queue,
    ));

    #[cfg(target_os = "android")]
    let adapter = Rc::new(SlintServoAdapter::new(
        app_weak,
        waker_sender.clone(),
        waker_receiver.clone(),
    ));

    init_servo_webview(adapter.clone(), "https://slint.dev".into());

    spin_servo_event_loop(adapter.clone());

    on_app_callbacks(adapter.clone());

    app.run().expect("Application failed to run - check for runtime errors");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(android_app: slint::android::AndroidApp) {
    slint::android::init(android_app).unwrap();
    main();
}
