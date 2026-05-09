<img src="https://github.com/user-attachments/assets/bcc2b6d9-b4f3-49a4-9653-8676eba4c2f0" width="100" align="right" />

# Slint Servo Example

> Disclaimer: Servo is still experimental and not ready for production use.

Integrate [Servo](https://github.com/servo/servo) Web Engine as WebView Component for Slint to render websites using hardware rendring on Linux, MacOS, Windows and Android.

![Preview](https://github.com/user-attachments/assets/a7259d9c-2d3a-4f7c-9f48-8fb852f6c5be)

## Prerequisites

- [UV](https://docs.astral.sh/uv/)

## Simple Usage

- Copy `webview.slint` from [src](https://github.com/slint-ui/slint/tree/master/examples/servo/src) and paste it in your project.
- Use `Webview` to your `.slint` file.

    ```slint
    import { Webview } from "webview.slint";

    export component MyApp inherits Window {
    ...
        Rectangle {
            Webview {
                width: 100%;
                height: 100%;
                changed current_url => {
                    // example usage
                    // lineEdit.text = self.current_url;
                }
            }
        }
    ...
    }
    ```

- Initialize it in your app with below code.

    ```rust
    pub mod webview;

    use slint::ComponentHandle;

    use crate::webview::WebView;

    slint::include_modules!();

    pub fn main() {
        setup_slint_with_wgpu();

        let app = MyApp::new().unwrap();

        let initialized = Cell::new(false);
        let app_weak = app.as_weak();

        app.window()
            .set_rendering_notifier(move |state, graphics_api| {
                if !matches!(state, slint::RenderingState::RenderingSetup) || initialized.get() {
                    return;
                }
                let slint::GraphicsAPI::WGPU28 { device, queue, .. } = graphics_api else {
                    panic!(
                        "Slint did not select a wgpu-28 renderer; \
                        enable a wgpu-capable renderer feature"
                    );
                };
                let app = app_weak.upgrade().unwrap();
                WebView::new(app, "https://slint.dev".into(), device.clone(), queue.clone());
                initialized.set(true);
            }).unwrap();

        app.run().unwrap();
    }

    fn setup_slint_with_wgpu() {
        use slint::wgpu_28::{WGPUConfiguration, WGPUSettings};

        #[allow(unused_mut)]
        let mut wgpu_settings = WGPUSettings::default();

        #[cfg(target_os = "windows")]
        {
            wgpu_settings.backends = slint::wgpu_28::wgpu::Backends::DX12;
        }

        slint::BackendSelector::new()
            .require_wgpu_28(WGPUConfiguration::Automatic(wgpu_settings))
            .select()
            .unwrap();
    }
    ```

## Prerequisites for Windows

To build on Windows, you will need Visual Studio installed. Cargo requires the `LIBCLANG_PATH` environment variable set to the LLVM tools directory bundled with Visual Studio.

- Install [Visual Studio](https://visualstudio.microsoft.com/downloads/).
- Install [C++ build tools using Visual Studio Installer](https://visualstudio.microsoft.com/visual-cpp-build-tools/).
- Set environment variable `LIBCLANG_PATH` for example "C:\\Program Files\\Microsoft Visual Studio\\18\\Community\\VC\\Tools\\Llvm\\x64\\bin"

## For Android build

- Update your code with android specific code from [example](https://github.com/slint-ui/slint/tree/master/examples/servo/src) to your project.

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

platform: linux-x86_64 (Linux) | darwin-x86_64 (Mac) | windows-x86_64 (Windows)

```bash
export BINDGEN_EXTRA_CLANG_ARGS="--target=aarch64-linux-android30 --sysroot=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/<platform>/sysroot"
```

### Run on android emulator or device

```bash
cargo apk run --target aarch64-linux-android --lib
```
