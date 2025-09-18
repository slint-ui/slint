<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Slint MCU backend

See also the [MCU docs](../../api/rs/slint/mcu.md)

## How to use

This crate re-export a `entry` attribute macro to apply to the `main` function, and a `init()`
function that should be called before creating the Slint UI.

In order to use this backend, the final program must depend on both `slint` and `mcu-board-support`.
The main.rs will look something like this

```rust,ignore
#![no_std]
#![cfg_attr(not(feature = "simulator"), no_main)]
slint::include_modules!();

#[allow(unused_imports)]
use mcu_board_support::prelude::*;

#[mcu_board_support::entry]
fn main() -> ! {
    mcu_board_support::init();
    MainWindow::new().run();
    panic!("The event loop should not return");
}
```

Since mcu-board-support is at the moment an internal crate not uploaded to crates.io, you must
use the git version of slint, slint-build, and mcu-board-support

```toml
[dependencies]
slint = { git = "https://github.com/slint-ui/slint", default-features = false }
mcu-board-support = { git = "https://github.com/slint-ui/slint" }
# ...
[build-dependencies]
slint-build = { git = "https://github.com/slint-ui/slint" }
```

In your build.rs, you must include a call to `slint_build::print_rustc_flags().unwrap()` to set some of the flags.

## Run the demo:

### The simulator


```sh
cargo run -p printerdemo_mcu --features=simulator --release
```

### On the Raspberry Pi Pico

Build the demo with:

```sh
cargo build -p printerdemo_mcu --no-default-features --features=mcu-board-support/pico-st7789 --target=thumbv6m-none-eabi --release
```

The resulting file can be flashed conveniently with [elf2uf2-rs](https://github.com/jonil/elf2uf2-rs). Install it using `cargo install`:

```sh
cargo install elf2uf2-rs
```

Then upload the demo to the Raspberry Pi Pico: push the "bootsel" white button on the device while connecting the
micro-usb cable to the device, this connect some storage where you can store the binary.

Or from the command on linux: (connect the device while pressing the "bootsel" button.

```sh
# If you're on Linux: mount the device
udisksctl mount -b /dev/sda1
# upload
elf2uf2-rs -d target/thumbv6m-none-eabi/release/printerdemo_mcu
```

### On the Raspberry Pi Pico2

Build the demo with:

```sh
cargo build -p printerdemo_mcu --no-default-features --features=mcu-board-support/pico2-st7789 --target=thumbv8m.main-none-eabihf --release
```

The resulting file can be flashed conveniently with [picotool](https://github.com/raspberrypi/picotool). You should build it from source.

Then upload the demo to the Raspberry Pi Pico: push the "bootsel" white button on the device while connecting the
micro-usb cable to the device, this connects some USB storage on your workstation where you can store the binary.

Or from the command on linux (connect the device while pressing the "bootsel" button):

```sh
# If you're on Linux: mount the device
udisksctl mount -b /dev/sda1
# upload
picotool load -u -v -x -t elf target/thumbv8m.main-none-eabihf/release/printerdemo_mcu
```

#### Using probe-rs

This requires [probe-rs](https://probe.rs) and to connect the pico via a probe
(for example another pico running the probe).

Then you can simply run with `cargo run`

```sh
CARGO_TARGET_THUMBV6M_NONE_EABI_LINKER="flip-link" CARGO_TARGET_THUMBV6M_NONE_EABI_RUNNER="probe-rs run --chip RP2040" cargo run -p printerdemo_mcu --no-default-features --features=mcu-board-support/pico-st7789 --target=thumbv6m-none-eabi --release
```

#### Flashing and Debugging the Pico with `probe-rs`'s VSCode Plugin

Install `probe-rs-debugger` and the VSCode plugin as described [here](https://probe.rs/docs/tools/vscode/).

Add this build task to your `.vscode/tasks.json`:
```json
{
	"version": "2.0.0",
	"tasks": [
		{
			"type": "cargo",
			"command": "build",
			"args": [
				"--package=printerdemo_mcu",
				"--no-default-features",
				"--features=mcu-board-support/pico-st7789",
				"--target=thumbv6m-none-eabi",
				"--profile=release-with-debug"
			],
			"problemMatcher": [
				"$rustc"
			],
			"group": "build",
			"label": "build mcu demo for pico"
		},
	]
}
```

The `release-with-debug` profile is needed, because the debug build does not fit into flash.

You can define it like this in your top level `Cargo.toml`:

```toml
[profile.release-with-debug]
inherits = "release"
debug = true
```

Now you can add the launch configuration to `.vscode/launch.json`:

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "preLaunchTask": "build mcu demo for pico",
            "type": "probe-rs-debug",
            "request": "launch",
            "name": "Flash and Debug MCU Demo",
            "cwd": "${workspaceFolder}",
            "connectUnderReset": false,
            "chip": "RP2040",
            "flashingConfig": {
                "flashingEnabled": true,
                "resetAfterFlashing": true,
                "haltAfterReset": true
            },
            "coreConfigs": [
                {
                    "coreIndex": 0,
                    "rttEnabled": true,
                    "programBinary": "./target/thumbv6m-none-eabi/release-with-debug/printerdemo_mcu"
                }
            ]
        },
    ]
}
```

This was tested using a second Raspberry Pi Pico programmed as a probe with [DapperMime](https://github.com/majbthrd/DapperMime).

### STM32H735G-DK

Using [probe-rs](https://probe.rs).

```sh
CARGO_PROFILE_RELEASE_OPT_LEVEL=s CARGO_TARGET_THUMBV7EM_NONE_EABIHF_RUNNER="probe-rs run --chip STM32H735IGKx" cargo run -p printerdemo_mcu --no-default-features  --features=mcu-board-support/stm32h735g --target=thumbv7em-none-eabihf --release
```

### STM32U5G9J-DK2

 cargo build -p mcu-board-support --target=thumbv8m.main-none-eabihf --features stm32u5g9j-dk2 --no-default-features
Using [probe-rs](https://probe.rs).

```sh
CARGO_PROFILE_RELEASE_OPT_LEVEL=s CARGO_TARGET_THUMBV8M_MAIN_NONE_EABIHF_RUNNER="probe-rs run --chip STM32U5G9ZJTxQ" cargo run -p printerdemo_mcu --no-default-features  --features=mcu-board-support/stm32u5g9j-dk2 --target=thumbv8m.main-none-eabihf --release
```

### ESP32

#### Prerequisites

 * ESP Rust Toolchain: https://esp-rs.github.io/book/installation/installation.html
 * `espflash`: Install via `cargo install espflash`.

When flashing, with `esplash`, you will be prompted to select a USB port. If this port is always the same, then you can also pass it as a parameter on the command line to avoid the prompt. For example if
`/dev/ttyUSB1` is the device file for your port, the command line changes to `espflash --monitor /dev/ttyUSB1 path/to/binary/to/flash_and_monitor`.

#### ESP32-S3-Box

The ESP32-S3-Box development board features:
- 2.4" LCD display with 320x240 resolution
- ILI9486 display controller
- GT911 capacitive touch controller
- ESP32-S3 with built-in WiFi and Bluetooth

To compile and run the demo:

```sh
CARGO_PROFILE_RELEASE_OPT_LEVEL=s cargo +esp run -p printerdemo_mcu --target xtensa-esp32s3-none-elf --no-default-features --features=mcu-board-support/esp32-s3-box-3 --release --config examples/mcu-board-support/esp32_s3_box_3/cargo-config.toml
```

#### ESP32-S3-LCD-EV-Board

The ESP32-S3-LCD-EV-Board development board features:
- 4.3" LCD display with 480x480 resolution
- RGB interface display
- FT5x06 capacitive touch controller
- ESP32-S3 with built-in WiFi and Bluetooth

To compile and run the demo:

```sh
CARGO_PROFILE_RELEASE_OPT_LEVEL=s cargo +esp run -p printerdemo_mcu --target xtensa-esp32s3-none-elf --no-default-features --features=mcu-board-support/esp32-s3-lcd-ev-board --release --config examples/mcu-board-support/esp32_s3_lcd_ev_board/cargo-config.toml
```

#### Waveshare ESP32-S3 Touch AMOLED 1.8"

The Waveshare ESP32-S3 Touch AMOLED 1.8" board features:
- 1.8" AMOLED display with 368x448 resolution
- SH8601 display controller
- FT3168 capacitive touch controller (touch support TODO)
- ESP32-S3 with 16MB flash and 8MB PSRAM

To compile and run the demo:

```sh
CARGO_PROFILE_RELEASE_OPT_LEVEL=s cargo +esp run -p printerdemo_mcu --target xtensa-esp32s3-none-elf --no-default-features --features=mcu-board-support/waveshare-esp32-s3-touch-amoled-1-8 --release --config examples/mcu-board-support/waveshare_esp32_s3_touch_amoled_1_8/cargo-config.toml
```

#### M5Stack CoreS3

The M5Stack CoreS3 development board features:
- 2.0" capacitive-touch IPS panel with 320x240 resolution
- ILI9342C display controller
- FT6336 capacitive touch controller (currently disabled - display-only mode)
- ESP32-S3 with 16MB flash and 8MB PSRAM
- AXP2101 power management unit (critical for proper operation)
- Built-in camera, IMU, magnetometer, and RTC

The M5Stack CoreS3 requires proper power management initialization via the AXP2101 PMU.
This is handled automatically by the board support.

Note: Touch support is temporarily disabled until a proper FT6336U driver is available.
The board currently operates in display-only mode.

To compile and run the demo:

```sh
CARGO_PROFILE_RELEASE_OPT_LEVEL=s cargo +esp run -p printerdemo_mcu --target xtensa-esp32s3-none-elf --no-default-features --features=mcu-board-support/m5stack-cores3 --release --config examples/mcu-board-support/m5stack_cores3/cargo-config.toml
```

#### ESoPe SLD_C_W_S3

The [ESoPE SLD_C_W_S3](https://esope.de/de/produkte/esope-plattform?view=article&id=95:pr-sld-c-w-s3-de&catid=11) PCB features an ESP32 S3,
for use in combination with [Smartwin displays](https://shop.schukat.com/de/de/EUR/c/ESOP) from Schukat.

To compile and run the demo:

```sh
CARGO_PROFILE_RELEASE_OPT_LEVEL=s cargo +esp run -p printerdemo_mcu --target xtensa-esp32s3-none-elf --no-default-features --features=mcu-board-support/esope-sld-c-w-s3 --release --config examples/mcu-board-support/esope_sld_c_w_s3/cargo-config.toml
```

