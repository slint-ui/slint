// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub mod webview;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod gl_bindings {
    #![allow(unsafe_op_in_unsafe_fn)]

    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

use slint::ComponentHandle;

use crate::webview::WebView;

slint::include_modules!();

pub fn main() {
    let (device, queue) = setup_wgpu();

    let app = MyApp::new().expect("Failed to create Slint application - check UI resources");

    WebView::new(app.clone_strong(), "https://slint.dev".into(), device, queue);

    app.run().expect("Application failed to run - check for runtime errors");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(android_app: slint::android::AndroidApp) {
    slint::android::init(android_app).unwrap();
    main();
}

fn setup_wgpu() -> (wgpu::Device, wgpu::Queue) {
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
        .require_wgpu_28(slint::wgpu_28::WGPUConfiguration::Manual {
            instance,
            adapter,
            device: device.clone(),
            queue: queue.clone()
        })
        .select()
        .expect("Failed to create Slint backend with WGPU based renderer - ensure your system supports WGPU");

    (device, queue)
}
