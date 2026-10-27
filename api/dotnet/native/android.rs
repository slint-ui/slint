// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The entry point on Android, like `slint_main` in the C++ API.
//! `NativeActivity` loads this library and runs `android_main` on a thread of its own,
//! which becomes the Slint thread and calls the C# entry point.

use std::sync::OnceLock;

use i_slint_backend_android_activity::{AndroidApp, AndroidPlatform};

static MAIN: OnceLock<extern "C" fn()> = OnceLock::new();

/// Sets the C# entry point. Callable from any thread, before `NativeActivity.onCreate`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_android_set_main(main: extern "C" fn()) {
    let _ = MAIN.set(main);
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    i_slint_core::platform::set_platform(Box::new(AndroidPlatform::new(app)))
        .expect("the Android platform is set once");
    crate::check_thread().expect("android_main runs before any other use of Slint");
    let main =
        MAIN.get().expect("slint_dotnet_android_set_main runs before NativeActivity.onCreate");
    main();
}
