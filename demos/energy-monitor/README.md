<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Energy Monitor Demo

![Energy Monitor Demo Screenshot](https://slint.dev/resources/energy-monitor-screenshot.png "Energy Monitor")

This is a demonstration of the Slint toolkit. This demo can be executed on various platforms.

## Displaying Real-Time Weather Data

To showcase real-time weather data, you will need an application key from https://www.weatherapi.com/. You can inject the API key by setting the `WEATHER_API` environment variable. The geographical location for the weather data can be set using the `WEATHER_LAT` and `WEATHER_LONG` variables. By default, the location is set to Berlin.

## Platform Compatibility

### Desktop (Windows/Mac/Linux) or Embedded Linux

You can run the demo on a desktop or embedded Linux environment with the following command:
```sh
cargo run -p energy-monitor
```

### Microcontrollers (MCU)

Refer to the [MCU backend Readme](../../examples/mcu-board-support) for instructions on how to run the demo on smaller devices like the Raspberry Pi Pico.

To run the MCU-like code on desktop, use the `--features=simulator`

```sh
cargo run -p energy-monitor --no-default-features --features=simulator --release
```

### Android

First, [set up your Android environment](https://slint.dev/snapshots/master/docs/rust/slint/android/#building-and-deploying).
Then, you can run the demo on an Android device with the following command:

```sh
cargo apk run -p energy-monitor --target aarch64-linux-android --lib
```

### Web

```sh
cargo install wasm-pack
cd demos/energy-monitor/
wasm-pack build --release --target web --no-default-features --features slint/default,chrono
python3 -m http.server
```

### Building and running on iOS

This demo can be cross-compiled to iOS to run on iPhones, iPads, and the respective simulators.

#### Prerequisites

 - A computer running macOS.
 - An up-to-date installation of [Xcode](https://developer.apple.com/xcode/).
 - [Xcodegen](https://github.com/yonaskolb/XcodeGen?tab=readme-ov-file#installing)
 - [Rust](https://rustup.rs). Add the target and simulator toolchains using `rustup target add aarch64-apple-ios` and `rustup target add aarch64-apple-ios-sim`

#### Building

1. Run `xcodegen -s ios-project.yml` to generate an XCode project file (`.xcodeproj`).
2. Open XCode and open the generated `.xcodeproj` in it.
3. Run, deploy, and debug the demo from within Xcode.

