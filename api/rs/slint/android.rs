// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore javac
//! Android backend.
//!
//! **Note:** This module is only available on Android with the "backend-android-activity-06" feature
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
//! Do not forget the `#[unsafe(no_mangle)]`
//!
//! ```rust
//! # #[cfg(target_os = "android")]
//! #[unsafe(no_mangle)]
//! fn android_main(app: slint::android::AndroidApp) {
//!     slint::android::init(app).unwrap();
//!
//!     // ... rest of your code ...
//!     slint::slint!{
//!         #[edition = "1.20"]
//!         export component MainWindow inherits Window {
//!             Text { text: "Hello World"; }
//!         }
//!     }
//!     MainWindow::new().unwrap().run().unwrap();
//! }
//! ```
//!
//! That function must be in a `cdylib` library, and you should enable the "backend-android-activity-06"
//! feature of the slint crate in your Cargo.toml:
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! slint = { version = "1.19.0", features = ["backend-android-activity-06"] }
//! ```
//!
//! ## Building and Deploying
//!
//! Building a Rust application requires the target toolchain to be installed. You can install it via `rustup`. For example, to target AArch64 Android, use the following command:
//!
//! ```sh
//! rustup target add aarch64-linux-android
//! ```
//!
//! Make sure that you have the Android NDK and SDK installed and set up in your development environment.
//! For detailed instructions on how to set up the Android NDK and SDK, please refer to the [Android Developer's guide](https://developer.android.com/studio/projects/install-ndk).
//! The following environment variables need to be set:
//! * `ANDROID_HOME`: The directory in which your Android SDK is located. Usually `$HOME/Android/Sdk`.
//! * `ANDROID_NDK_ROOT`: The directory in which your Android NDK is located. Usually `$HOME/Android/Sdk/ndk/${NDK_VERSION}`. ${NDK_VERSION} is the version of the NDK you have installed.
//! * `JAVA_HOME`: The directory in which your Java compiler (`javac`) is located. This variable is optional if a `javac` is found in your `$PATH`.
//!   Otherwise you can set `JAVA_HOME` to the `javac` installation shipped with Android Studio in `android-studio/jbr`.
//!
//! Slint doesn't require a specific tool to build, sign, and deploy the APK.
//! The APK's activity must be `android.app.NativeActivity`, or a subclass of it,
//! because that's the activity type the "backend-android-activity-06" feature supports.
//! Tools that can package such an APK include:
//!
//! * [cargo-apk2](https://crates.io/crates/cargo-apk2): `cargo apk2 run --target aarch64-linux-android --lib`
//! * [cargo-apk](https://crates.io/crates/cargo-apk): `cargo apk run --target aarch64-linux-android --lib`
//! * [cargo-ndk](https://crates.io/crates/cargo-ndk) to build the library, with Gradle to package it
//! * [xbuild](https://github.com/rust-mobile/xbuild): `x run --device <id>`
//!
#![doc = i_slint_core_macros::slint_doc_str!("See the [Android platform guide](slint:android_building) for how to set up each of them.")]

/// Re-export of the android-activity crate.
#[cfg(all(
    target_os = "android",
    any(feature = "backend-android-activity-05", feature = "backend-android-activity-06")
))]
pub use i_slint_backend_android_activity::android_activity;

#[cfg(not(all(
    target_os = "android",
    any(feature = "backend-android-activity-05", feature = "backend-android-activity-06")
)))]
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

/// Builds and installs the Android backend, with more control than [`init()`].
///
/// **Note:** This type is only functional on Android with the "backend-android-activity-06" feature
///
/// Call [`init()`](Self::init) from the `android_main` function before any call to Slint that
/// needs a backend.
///
/// ```rust
/// # #[cfg(target_os = "android")]
/// #[unsafe(no_mangle)]
/// fn android_main(app: slint::android_activity::AndroidApp) {
///     slint::android::BackendBuilder::new(app)
///         .with_back_callback_priority(None)
///         .with_event_listener(|event| eprintln!("got event {event:?}"))
///         .init()
///         .unwrap();
///
///     // ... rest of your application ...
/// }
/// ```
#[must_use = "call init() to install the backend"]
pub struct BackendBuilder {
    app: android_activity::AndroidApp,
    back_callback_priority: Option<Option<u32>>,
    event_listener: Option<alloc::boxed::Box<dyn Fn(&android_activity::PollEvent<'_>)>>,
}

impl BackendBuilder {
    /// Creates a builder with the same behavior as [`init()`].
    pub fn new(app: android_activity::AndroidApp) -> Self {
        Self { app, back_callback_priority: None, event_listener: None }
    }

    /// Sets the priority of the `OnBackInvokedCallback` that Slint registers on Android 14
    /// (API 34) and later.
    ///
    /// The callback passes Back to the items as a key event and finishes the activity if no
    /// item accepts it. Android calls the callback with the highest priority first, so an
    /// application that registers its own callback can choose a priority above or below Slint's.
    /// The default is `Some(0)`, which is `OnBackInvokedDispatcher.PRIORITY_DEFAULT`.
    /// Values above `i32::MAX` count as `i32::MAX`.
    ///
    /// `None` registers no callback: Back reaches the application's own handler,
    /// and Slint items don't see it as a key event.
    ///
    /// This has no effect before Android 14.
    pub fn with_back_callback_priority(mut self, priority: Option<u32>) -> Self {
        self.back_callback_priority = Some(priority);
        self
    }

    /// Notifies `listener` of the [`android_activity::PollEvent`]s, as [`init_with_event_listener()`] does.
    pub fn with_event_listener(
        mut self,
        listener: impl Fn(&android_activity::PollEvent<'_>) + 'static,
    ) -> Self {
        self.event_listener = Some(alloc::boxed::Box::new(listener));
        self
    }

    /// Installs the backend as Slint's platform.
    ///
    /// **Note:** This function is only available on Android with the "backend-android-activity-06" feature
    pub fn init(self) -> Result<(), SetPlatformError> {
        #[cfg(not(target_os = "android"))]
        {
            let _ = self;
            unreachable!();
        }
        #[cfg(target_os = "android")]
        {
            use i_slint_backend_android_activity::AndroidPlatform;
            let mut platform = match self.back_callback_priority {
                Some(priority) => {
                    AndroidPlatform::new_with_back_callback_priority(self.app, priority)
                }
                None => AndroidPlatform::new(self.app),
            };
            if let Some(listener) = self.event_listener {
                platform = platform.with_event_listener(listener);
            }
            crate::platform::set_platform(alloc::boxed::Box::new(platform))?;
            #[cfg(any(feature = "mcp", feature = "system-testing"))]
            i_slint_backend_selector::init_testing_backends();
            Ok(())
        }
    }
}

/// Initializes the Android backend.
///
/// **Note:** This function is only available on Android with the "backend-android-activity-06" feature
///
/// This function must be called from the `android_main` function before any call to Slint that needs a backend.
///
/// See the [module documentation](self) for an example on how to create Android application.
///
/// See also [`init_with_event_listener`] and [`BackendBuilder`] for more options.
pub fn init(app: android_activity::AndroidApp) -> Result<(), SetPlatformError> {
    BackendBuilder::new(app).init()
}

/// Similar to [`init()`], which allow to listen to android-activity's event
///
/// This is a shortcut for `BackendBuilder::new(app).with_event_listener(listener).init()`.
///
/// **Note:** This function is only available on Android with the "backend-android-activity-06" feature
///
/// The listener argument is a function that takes a [`android_activity::PollEvent`](https://docs.rs/android-activity/latest/android_activity/enum.PollEvent.html)
///
/// # Example
///
/// ```rust
/// # #[cfg(target_os = "android")]
/// #[unsafe(no_mangle)]
/// fn android_main(app: slint::android_activity::AndroidApp) {
///     slint::android::init_with_event_listener(
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
    BackendBuilder::new(app).with_event_listener(listener).init()
}
