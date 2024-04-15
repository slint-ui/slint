// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! Android backend.
//!
//! **Note:** This module is only available on Android with the "backend-android-activity-05" feature
//!
//! Slint uses the [android-activity crate](https://github.com/rust-mobile/android-activity) as a backend.
//!
//! For convenience, Slint re-exports the content of the [`android-activity`](https://docs.rs/android-activity)  under `slint::android::android_activity`.
//!
//! As with every application using the android-activity crate, the entry point to your app will be the `android_main` function.
//! From that function, you can call [`slint::android::init`](init()) or [`slint::android::init_with_event_listener`](init_with_event_listener)
//!
//! # Example
//!
//! This is a basic example of an Android application.
//! Do not forget the `#[no_mangle]`
//!
//! ```rust
//! # #[cfg(target_os = "android")]
//! #[no_mangle]
//! fn android_main(app: slint::android::AndroidApp) {
//!     slint::android::init(app).unwrap();
//!
//!     // ... rest of your code ...
//!     slint::slint!{
//!         export component MainWindow inherits Window {
//!             Text { text: "Hello World"; }
//!         }
//!     }
//!     MainWindow::new().unwrap().run().unwrap();
//! }
//! ```
//!
//! That function must be in a `cdylib` library, and you should enable the "backend-android-activity-05"`
//! feature of the slint crate in your Cargo.toml:
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! slint = { version = "1.5", features = ["backend-android-activity-05"] }
//! ```
//!
//! ## Building and Deploying
//!
//! To build and deploy your application, we suggest the usage of [cargo-apk](https://github.com/rust-mobile/cargo-apk),
//! a cargo subcommand that allows you to build, sign, and deploy Android APKs made in Rust.
//!
//! You can install it and use it with the following command:
//!
//! ```sh
//! cargo install cargo-apk
//! cargo apk run --target aarch64-linux-android --lib
//! ```
//!
//! Please ensure that you have the Android NDK and SDK installed and properly set up in your development environment for the above command to work as expected.
//! For detailed instructions on how to set up the Android NDK and SDK, please refer to the [Android Developer's guide](https://developer.android.com/studio/projects/install-ndk).
//! The `ANDROID_HOME` and `ANDROID_NDK_ROOT` environment variable need to be set to the right path.
//!
//! Note Slint does not require a specific build tool and can work with others, such as [xbuild](https://github.com/rust-mobile/xbuild).

/// Re-export of the android-activity crate.
#[cfg(all(target_os = "android", feature = "backend-android-activity-05"))]
pub use i_slint_backend_android_activity::android_activity;

#[cfg(not(all(target_os = "android", feature = "backend-android-activity-05")))]
/// Re-export of the [android-activity](https://docs.rs/android-activity) crate.
pub mod android_activity {
    #[doc(hidden)]
    pub struct AndroidApp;
    #[doc(hidden)]
    pub struct PollEvent<'a>(&'a ());
}

/// Re-export of AndroidApp from the [android-activity](https://docs.rs/android-activity) crate.
#[doc(no_inline)]
pub use android_activity::AndroidApp;

use crate::platform::SetPlatformError;

/// Initializes the Android backend.
///
/// **Note:** This function is only available on Android with the "backend-android-activity-05" feature
///
/// This function must be called from the `android_main` function before any call to Slint that needs a backend.
///
/// See the [module documentation](self) for an example on how to create Android application.
///
/// See also [`init_with_event_listener`]
pub fn init(app: android_activity::AndroidApp) -> Result<(), SetPlatformError> {
    crate::platform::set_platform(Box::new(i_slint_backend_android_activity::AndroidPlatform::new(
        app,
    )))
}

/// Similar to [`init()`], which allow to listen to android-activity's event
///
/// **Note:** This function is only available on Android with the "backend-android-activity-05" feature
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
/// Check out the [module documentation](self) for a more complete example  on how to write an android application
pub fn init_with_event_listener(
    app: android_activity::AndroidApp,
    listener: impl Fn(&android_activity::PollEvent<'_>) + 'static,
) -> Result<(), SetPlatformError> {
    crate::platform::set_platform(Box::new(
        i_slint_backend_android_activity::AndroidPlatform::new_with_event_listener(app, listener),
    ))
}
