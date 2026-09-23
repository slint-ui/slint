<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->
<!-- cspell:ignore androidscrollcomparison worktree -->

# Android and Slint scroll physics comparison

This manual Android harness superimposes a translucent native `ScrollView` over
a Slint `ScrollView`.
It forwards a copy of each `MotionEvent`, including its event time and
historical samples, through a diagnostic JNI bridge to Slint.
Both implementations therefore receive the same gesture while a live overlay
shows their offsets, percentage difference, frame velocities, and maximum
separation.

A native/native control mode forwards each `MotionEvent` from one Android list
to another to validate the event-copying setup.

The native pane and JNI bridge run inside Slint's Android activity so both
lists share the same device input and frame clock.
This comparison branch includes the diagnostic runtime instrumentation, so a
normal checkout and build displays both views without an additional patch step.

## Setup

```sh
rustup target add aarch64-linux-android
cd tests/manual/android-scroll-physics-comparison
CARGO_APK_RELEASE_KEYSTORE="$HOME/.android/debug.keystore" \
CARGO_APK_RELEASE_KEYSTORE_PASSWORD=android \
  cargo apk build --release --target-dir ../../../target
```

Set `ANDROID_HOME` and `ANDROID_NDK_HOME` if they are not already configured.
The signing variables above use Android Studio's conventional local debug key
for a Release-optimized diagnostic APK; substitute another local key if needed.

Install the APK path reported by `cargo apk`, then run the standard gesture
matrix from the repository root:

```sh
adb -s "$ADB_SERIAL" install -r <apk-path>
ADB_SERIAL="$ADB_SERIAL" \
  tests/manual/android-scroll-physics-comparison/scripts/run-matrix.sh
```

Set `OUTPUT_DIR` to override the default trace directory
`/tmp/android-scroll-traces`.

To validate event forwarding, launch native/native control mode:

```sh
adb -s "$ADB_SERIAL" shell am start \
  -n dev.slint.androidscrollcomparison/android.app.NativeActivity \
  --ez native_control true
```

The standard matrix includes a three-trial short, hard flick that moves 120 dp
in 20 ms.
The diagnostic instrumentation records native and Slint frame offsets, native
release velocity, Slint's estimated release velocity, and the interval between
the last move event and release.
The current checkout preserves the leading historical movement segment and original sample timing.
Non-bouncing touch flings use Slint's standalone Rust implementation of the AOSP spline.
Android `OverScroller` is only a comparison reference; it does not drive Slint motion.
Keep the device awake and unlocked while running the matrix.
See the latest validation section in [FINDINGS.md](FINDINGS.md) for measured alignment and remaining limitations.
