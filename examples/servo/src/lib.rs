// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub mod webview;

#[cfg(any(target_os = "linux", target_os = "android", target_os = "windows"))]
mod gl_bindings {
    #![allow(unsafe_op_in_unsafe_fn)]

    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

use std::cell::Cell;

use slint::ComponentHandle;

use crate::webview::WebView;

slint::include_modules!();

pub fn main() {
    setup_slint_with_wgpu();

    let app = MyApp::new().expect("Failed to create Slint application - check UI resources");

    let initialized = Cell::new(false);
    let app_weak = app.as_weak();

    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            if !matches!(state, slint::RenderingState::RenderingSetup) || initialized.get() {
                return;
            }
            let slint::GraphicsAPI::WGPU28 { device, queue, .. } = graphics_api else {
                panic!(
                    "Slint did not select a wgpu-28 renderer; \
                     enable a wgpu-capable renderer feature"
                );
            };
            let app = app_weak.upgrade().expect("App dropped before rendering setup");
            WebView::new(app, "https://slint.dev".into(), device.clone(), queue.clone());
            initialized.set(true);
        })
        .expect("Failed to install rendering notifier");

    app.run().expect("Application failed to run - check for runtime errors");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(android_app: slint::android::AndroidApp) {
    slint::android::init(android_app).unwrap();
    main();
}

fn setup_slint_with_wgpu() {
    #[allow(unused_mut)]
    let mut wgpu_settings = slint::wgpu_28::WGPUSettings::default();

    #[cfg(target_os = "windows")]
    {
        // Must be DX12 on Windows to support texture sharing from ANGLE's D3D11 via NT handles.
        wgpu_settings.backends = slint::wgpu_28::wgpu::Backends::DX12;
    }

    slint::BackendSelector::new()
        .require_wgpu_28(slint::wgpu_28::WGPUConfiguration::Automatic(wgpu_settings))
        .select()
        .expect(
            "Failed to create Slint backend with WGPU based renderer - \
             ensure your system supports WGPU",
        );
}
