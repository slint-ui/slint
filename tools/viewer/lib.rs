// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// This file exists only to expose `android_main` from the cdylib for Android's
// NativeActivity. The viewer's command-line implementation lives in main.rs.

#[cfg(all(target_os = "android", not(feature = "remote")))]
compile_error!("The `remote` feature is required when building for Android");

#[cfg(all(target_os = "android", feature = "remote"))]
mod remote;

#[cfg(all(target_os = "android", feature = "remote"))]
#[unsafe(no_mangle)]
fn android_main(app: i_slint_backend_android_activity::android_activity::AndroidApp) {
    i_slint_core::platform::set_platform(Box::new(
        i_slint_backend_android_activity::AndroidPlatform::new(app),
    ))
    .unwrap();
    remote::run(None, true).unwrap();
}
