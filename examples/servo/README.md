<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Slint Servo Example

Integrate [Servo](https://github.com/servo/servo) Web Engine as WebView Component for Slint to render websites using handrware rendring on desktop platforms and software rendring on android for now.

<img width="2900" height="2052" alt="CleanShot 2025-10-28 at 12 58 30@2x" src="https://github.com/user-attachments/assets/a7259d9c-2d3a-4f7c-9f48-8fb852f6c5be" />

## For Android build on Mac

### Install Android Studio and JDK

```bash
brew install android-studio openjdk@17
```

### Set these to .zshrc

```bash
export JAVA_HOME="/opt/homebrew/opt/openjdk@17"
export PATH=$JAVA_HOME/bin:$PATH

export ANDROID_HOME=~/Library/Android/sdk
export ANDROID_SDK_ROOT=$ANDROID_HOME

export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358"
export ANDROID_NDK_ROOT=$ANDROID_NDK_HOME

export PATH=$ANDROID_HOME/tools:$PATH
export PATH=$ANDROID_HOME/platform-tools:$PATH
export PATH=$ANDROID_HOME/cmdline-tools/latest/bin:$PATH
```

### Install platofrm-tools, build-tools and ndk

```bash
sdkmanager platform-tools "platforms;android-30" "build-tools;34.0.0" "ndk;28.2.13676358"
```

### Add rust target anf install cargo apk

```bash
rustup target add aarch64-linux-android
cargo install cargo-apk
```

### Run on android emulator or device

```bash
export BINDGEN_EXTRA_CLANG_ARGS="--target=aarch64-linux-android30 --sysroot=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/sysroot"
cargo apk run --target aarch64-linux-android --lib
```
