<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial -->
**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.

# Slint Android Activity Backend

This crate implements the android backend/platform for Slint.

It uses the [android-activity](https://github.com/rust-mobile/android-activity) crate
to initialize the app and provide events handling.

At the moment, this is a work in progress. In the future, we expect to add features directly to the the slint crate.
In the mean time, it is already possible to use this crate to test Slint applications on Android.

## Usage

When using this crate into your project, be aware that it does not strictly adhere to semantic versioning (semver).
This means breaking changes may be introduced in any patch release.
It is crucial that the version of this crate matches the version of Slint you are using.
To specify the exact version of this crate, include the `=` symbol in the version string.

You are required to add either the `native-activity` or the `game-activity` feature.
The `native-activity` feature is a good starting point as it does not require Java stub.
However, it is more limited and may not work well with keyboard input.
For more details, refer to the [documentation of android-activity](https://github.com/rust-mobile/android-activity#should-i-use-nativeactivity-or-gameactivity).

To create an Android build, your crate must be a library with the `cdylib` crate-type.

Below is an example of how to set up your `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
slint = { version = "1.3.0", ... }
i-slint-backend-android-activity = { version = "=1.3.0", features = ["native-activity"] }
```

As with any application using `android-activity`, you need to implement the `android_init` function as `#[no_mangle]`.
In it, create a [`AndroidPlatform`] which you give to [`slint::platform::set_platform`][i_slint_core::platform::set_platform].

Here is an example:

```rust
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: i_slint_backend_android_activity::AndroidApp) {
    slint::platform::set_platform(Box::new(
        i_slint_backend_android_activity::AndroidPlatform::new(app)
    )).unwrap();

    // ... rest of your code ...
    slint::slint!{
        export component MainWindow inherits Window {
            Text { text: "Hello World"; }
        }
    }
    MainWindow::new().unwrap().run().unwrap();
}
```

## Building and Deploying

To build and deploy your application, we suggest the usage of [cargo-apk](https://github.com/rust-mobile/cargo-apk),
a cargo subcommand that allows you to build, sign, and deploy Android APKs made in Rust.

You can install it and use it with the following command:

```sh
cargo install cargo-apk
cargo apk run --target aarch64-linux-android --lib
```

Please ensure that you have the Android NDK and SDK installed and properly set up in your development environment for the above command to work as expected.
