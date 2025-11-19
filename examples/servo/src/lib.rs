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

use crate::servo_util::init_servo;

slint::include_modules!();

pub fn main() {
    #[cfg(not(target_os = "android"))]
    let (device, queue) = {
        let backends = wgpu::Backends::from_env().unwrap_or_default();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            flags: Default::default(),
            backend_options: Default::default(),
            memory_budget_thresholds: Default::default(),
        });

        let adapter = spin_on::spin_on(async {
            instance
                .request_adapter(&Default::default())
                .await
                .expect("Failed to find an appropriate WGPU adapter")
        });

        let (device, queue) = spin_on::spin_on(async {
            adapter.request_device(&Default::default()).await.expect("Failed to create WGPU device")
        });

        slint::BackendSelector::new()
        .require_wgpu_27(slint::wgpu_27::WGPUConfiguration::Manual {
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

    let url = "https://slint.dev";

    #[cfg(not(target_os = "android"))]
    let _adapter = init_servo(
        app.clone_strong(),
        url.into(),
        #[cfg(not(target_os = "android"))]
        device,
        #[cfg(not(target_os = "android"))]
        queue,
    );

    app.run().expect("Application failed to run - check for runtime errors");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(android_app: slint::android::AndroidApp) {
    slint::android::init(android_app).unwrap();
    main();
}
