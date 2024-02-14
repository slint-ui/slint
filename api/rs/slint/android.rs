// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

/// Re-export of the android_activity crate.
#[cfg(all(target_os = "android", feature = "backend-android-native-activity"))]
pub use i_slint_backend_android_activity::android_activity;

#[cfg(not(all(target_os = "android", feature = "backend-android-native-activity")))]
#[doc(hidden)]
mod android_activity {
    pub(crate) struct AndroidApp;
    pub(crate) struct PollEvent<'a>(&'a ());
}

use crate::platform::SetPlatformError;

/// Initializes the Android backend.
///
/// **Note:** This function is only available on Android with the "backend-android-native-activity" feature
///
/// This is meant to be called from the `android_main` function.
///
/// Slint uses the [android-activity crate](https://github.com/rust-mobile/android-activity) as a backend.
/// For convenience, Slint re-export the content of the [`android_activity`](https://docs.rs/android-activity)  under `slint::android_activity`
/// As every application using the android-activity crate, the entry point to your app will be the `android_main` function.
///
/// See also [`android_init_with_event_listener`]
///
/// # Example
///
/// This is a basic example of a rust application.
/// Do not forget the `#[no_mangle]`
///
/// ```rust
/// # #[cfg(target_os = "android")]
/// #[no_mangle]
/// fn android_main(app: slint::android_activity::AndroidApp) {
///     slint::android_init(app).unwrap();
///
///     // ... rest of your code ...
///     slint::slint!{
///         export component MainWindow inherits Window {
///             Text { text: "Hello World"; }
///         }
///     }
///     MainWindow::new().unwrap().run().unwrap();
/// }
/// ```
///
/// That function must be in a `cdylib` library, and you should enable the "backend-android-native-activity"`
/// feature of the slint crate in your Cargo.toml:
///
/// ```toml
/// [lib]
/// crate-type = ["cdylib"]
///
/// [dependencies]
/// slint = { version = "1.5", features = ["backend-android-native-activity"] }
/// ```
///
/// ## Building and Deploying
///
/// To build and deploy your application, we suggest the usage of [cargo-apk](https://github.com/rust-mobile/cargo-apk),
/// a cargo subcommand that allows you to build, sign, and deploy Android APKs made in Rust.
///
/// You can install it and use it with the following command:
///
/// ```sh
/// cargo install cargo-apk
/// cargo apk run --target aarch64-linux-android --lib
/// ```
///
/// Please ensure that you have the Android NDK and SDK installed and properly set up in your development environment for the above command to work as expected.
pub fn android_init(app: android_activity::AndroidApp) -> Result<(), SetPlatformError> {
    crate::platform::set_platform(Box::new(i_slint_backend_android_activity::AndroidPlatform::new(
        app,
    )))
}

/// Similar as [`android_init`], which allow to listen to android-activity's event
///
/// **Note:** This function is only available on Android with the "backend-android-native-activity" feature
///
/// The listener argument is a function that takes a [`android_activity::PollEvent`](https://docs.rs/android-activity/latest/android_activity/enum.PollEvent.html)
///
/// # Example
///
/// ```rust
/// # #[cfg(target_os = "android")]
/// #[no_mangle]
/// fn android_main(app: slint::android_activity::AndroidApp) {
///     slint::android_init_with_event_listener(
///        app,
///        |event| { eprintln!("got event {event:?}") }
///     ).unwrap();
///
///     // ... rest of your application ...
///
/// }
/// ```
///
/// Check out the documentation of [`android_init`] for a more complete example  on how to write an android application
pub fn android_init_with_event_listener(
    app: android_activity::AndroidApp,
    listener: impl Fn(&android_activity::PollEvent<'_>) + 'static,
) -> Result<(), SetPlatformError> {
    crate::platform::set_platform(Box::new(
        i_slint_backend_android_activity::AndroidPlatform::new_with_event_listener(app, listener),
    ))
}
