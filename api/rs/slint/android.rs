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

/// How the Android backend takes part in Back handling on Android 14 (API 34) and later.
///
/// Slint registers an `OnBackInvokedCallback` that passes Back to the items as a key event
/// and finishes the activity if no item accepts it.
/// An application that registers its own `OnBackInvokedCallback` can use this to
/// let its callback run instead.
/// Pass it to [`InitOptions::with_back_invoked_callback()`].
/// It has no effect before Android 14.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum BackInvokedCallback {
    /// Slint's default, which registers the callback with `OnBackInvokedDispatcher.PRIORITY_DEFAULT`.
    /// [`init()`] uses it.
    #[default]
    Default,
    /// Register the callback with this priority.
    /// Android calls the callback with the highest priority first.
    /// Values above `i32::MAX` count as `i32::MAX`.
    Priority(u32),
    /// Don't register a callback.
    /// Back reaches the application's own handler, and Slint items don't see it as a key event.
    Disabled,
}

/// Options for [`init_with_options()`].
///
/// ```rust
/// # #[cfg(target_os = "android")]
/// #[unsafe(no_mangle)]
/// fn android_main(app: slint::android_activity::AndroidApp) {
///     let options = slint::android::InitOptions::new()
///         .with_back_invoked_callback(slint::android::BackInvokedCallback::Disabled)
///         .with_event_listener(|event| eprintln!("got event {event:?}"));
///     slint::android::init_with_options(app, options).unwrap();
///
///     // ... rest of your application ...
/// }
/// ```
#[derive(Default)]
#[non_exhaustive]
pub struct InitOptions {
    back_invoked_callback: BackInvokedCallback,
    event_listener: Option<alloc::boxed::Box<dyn Fn(&android_activity::PollEvent<'_>)>>,
}

impl InitOptions {
    /// Creates options with the same behavior as [`init()`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets how the backend registers its `OnBackInvokedCallback`. See [`BackInvokedCallback`].
    #[must_use]
    pub fn with_back_invoked_callback(
        mut self,
        back_invoked_callback: BackInvokedCallback,
    ) -> Self {
        self.back_invoked_callback = back_invoked_callback;
        self
    }

    /// Notifies `listener` of the [`android_activity::PollEvent`]s, as [`init_with_event_listener()`] does.
    #[must_use]
    pub fn with_event_listener(
        mut self,
        listener: impl Fn(&android_activity::PollEvent<'_>) + 'static,
    ) -> Self {
        self.event_listener = Some(alloc::boxed::Box::new(listener));
        self
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
/// See also [`init_with_event_listener`] and [`init_with_options`]
pub fn init(app: android_activity::AndroidApp) -> Result<(), SetPlatformError> {
    init_with_options(app, InitOptions::new())
}

/// Similar to [`init()`], with the settings in [`InitOptions`].
///
/// **Note:** This function is only available on Android with the "backend-android-activity-06" feature
///
/// See [`InitOptions`] for an example.
pub fn init_with_options(
    app: android_activity::AndroidApp,
    options: InitOptions,
) -> Result<(), SetPlatformError> {
    #[cfg(not(target_os = "android"))]
    unreachable!();
    #[cfg(target_os = "android")]
    {
        use i_slint_backend_android_activity::{AndroidPlatform, DEFAULT_BACK_CALLBACK_PRIORITY};
        let priority = match options.back_invoked_callback {
            BackInvokedCallback::Default => Some(DEFAULT_BACK_CALLBACK_PRIORITY),
            BackInvokedCallback::Priority(p) => Some(p),
            BackInvokedCallback::Disabled => None,
        };
        let mut platform = AndroidPlatform::new_with_back_callback_priority(app, priority);
        if let Some(listener) = options.event_listener {
            platform = platform.with_event_listener(listener);
        }
        crate::platform::set_platform(alloc::boxed::Box::new(platform))?;
        #[cfg(any(feature = "mcp", feature = "system-testing"))]
        i_slint_backend_selector::init_testing_backends();
        Ok(())
    }
}

/// Similar to [`init()`], which allow to listen to android-activity's event
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
    init_with_options(app, InitOptions::new().with_event_listener(listener))
}
