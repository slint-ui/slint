<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->
<!-- cspell:ignore logcat -->

# Android and Slint Scroll Comparison

This app places a translucent Android `ScrollView` above a Slint `ScrollView`.
The native view forwards copies of its `MotionEvent` objects, including timestamps and historical positions, to Slint.
Both lists display their offsets and write frame samples to logcat under `ScrollCompare`.

The CI smoke test installs the optimized x86_64 APK on an Android emulator, performs one swipe, and checks that both offsets change.
It uploads logcat, a screenshot, and disk reports.
This test verifies gesture delivery, not scroll physics parity.

Build the app locally with `cargo apk build --manifest-path tests/manual/android-scroll-physics-comparison/Cargo.toml --target aarch64-linux-android --lib --release` for an ARM phone.
Set `ANDROID_HOME`, `ANDROID_NDK_HOME`, and the `CARGO_APK_RELEASE_KEYSTORE` signing variables for your Android installation.
