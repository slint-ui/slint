
# Slint Servo Example

> Disclaimer: Servo is still experimental and not ready for productions use.

Integrate [Servo](https://github.com/servo/servo) Web Engine as WebView Component for Slint to render websites using hardware rendring on MacOS, Linux and software rendring on android for now.

![Preview](https://github.com/user-attachments/assets/a7259d9c-2d3a-4f7c-9f48-8fb852f6c5be)

## Prerequisites

- [UV](https://docs.astral.sh/uv/)

## Simple Usage

- Copy webview from src and paste it in your project.
- Add `webview` to your `.slint` file.
- Initialize it in your app with below code.

```rust
pub mod webview;

use slint::ComponentHandle;

use crate::webview::WebView;

slint::include_modules!();

pub fn main() {
    let (device, queue) = setup_wgpu();

    let app = MyApp::new().unwrap();

    WebView::new(
        app.clone_strong(),
        "https://slint.dev".into(),
        device,
        queue,
    );

    app.run().unwrap();
}

fn setup_wgpu() -> (wgpu::Device, wgpu::Queue) {
    let backends = wgpu::Backends::from_env().unwrap_or_default();

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends,
        flags: Default::default(),
        backend_options: Default::default(),
        memory_budget_thresholds: Default::default(),
    });

    let adapter = spin_on::spin_on(async {
        instance
            .request_adapter(&Default::default())
            .await
            .unwrap()
    });

    let (device, queue) = spin_on::spin_on(async {
        adapter.request_device(&Default::default()).await.unwrap()
    });

    slint::BackendSelector::new()
        .require_wgpu_28(slint::wgpu_28::WGPUConfiguration::Manual {
            instance,
            adapter,
            device: device.clone(),
            queue: queue.clone()
        })
        .select()
        .unwrap();

    (device, queue)
}
```

## For Android build

- Update your code with android specific code from example to your project.

## Prerequisites for Android

- Install [JDK](https://www.oracle.com/java/technologies/downloads/).
- Install [Android Studio](https://developer.android.com/studio).
- Install [Android Command Line Tools](https://developer.android.com/studio#command-tools).

### Install platofrm-tools

```bash
${ANDROID_HOME}/cmdline-tools/latest/bin/sdkmanager --install "platforms;android-30"
```

### Add rust target and install cargo apk

```bash
rustup target add aarch64-linux-android
cargo install cargo-apk
```

### Setup Bindgen for Android

#### On Mac

```bash
export BINDGEN_EXTRA_CLANG_ARGS="--target=aarch64-linux-android30 --sysroot=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/sysroot"
```

#### On Linux

```bash
export BINDGEN_EXTRA_CLANG_ARGS="--target=aarch64-linux-android30 --sysroot=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/sysroot"
```

### Run on android emulator or device

```bash
cargo apk run --target aarch64-linux-android --lib
```
