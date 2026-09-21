<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->
<!-- cspell:ignore androidscrollcomparison worktree -->

# Android and Slint scroll physics comparison

This manual Android harness displays a native `ScrollView` beside a Slint
`ScrollView`. It records their offsets frame by frame for identical scripted
gestures. A native/native control mode forwards each `MotionEvent` from one
Android list to another to validate the event-copying setup.

The native pane must run inside Slint's Android activity so both lists receive
the same device input and share the same frame clock. The diagnostic
instrumentation is therefore supplied as a patch instead of being compiled
into the production backend.

## Setup

Use a disposable worktree because the instrumentation temporarily modifies
runtime sources:

```sh
git apply tests/manual/android-scroll-physics-comparison/patches/instrumentation.patch
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

Remove the temporary instrumentation when finished:

```sh
git apply -R tests/manual/android-scroll-physics-comparison/patches/instrumentation.patch
```

The instrumentation patch records native frame offsets, native release
velocity, Slint's estimated release velocity, and the interval between the
last move event and release. It does not include the candidate fix for the
missing leading Android history segment described in [FINDINGS.md](FINDINGS.md).
