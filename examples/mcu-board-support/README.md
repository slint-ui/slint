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

You need nightly rust because that's the only way to get an allocator.

Build the demo with:

```sh
cargo +nightly build -p printerdemo_mcu --no-default-features --features=mcu-board-support/pico-st7789 --target=thumbv6m-none-eabi --release
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

#### Using probe-run

This require [probe-run](https://github.com/knurling-rs/probe-run) (`cargo install probe-run`)
and to connect the pico via a probe (for example another pico running the probe)

Then you can simply run with `cargo run`

```sh
CARGO_TARGET_THUMBV6M_NONE_EABI_LINKER="flip-link" CARGO_TARGET_THUMBV6M_NONE_EABI_RUNNER="probe-run --chip RP2040" cargo +nightly run -p printerdemo_mcu --no-default-features --features=mcu-board-support/pico-st7789 --target=thumbv6m-none-eabi --release
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
			"env": {
				"RUSTUP_TOOLCHAIN": "nightly"
			},
			"args": [
				"--package=printerdemo_mcu",
				"--features=mcu-pico-st7789",
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

Using [probe-run](https://github.com/knurling-rs/probe-run) (`cargo install probe-run`)

```sh
CARGO_TARGET_THUMBV7EM_NONE_EABIHF_RUNNER="probe-run --chip STM32H735IGKx" cargo +nightly run -p printerdemo_mcu --no-default-features  --features=mcu-board-support/stm32h735g --target=thumbv7em-none-eabihf --release
```

### ESP32-S2-Kaluga-1

A esp toolchain is required: https://esp-rs.github.io/book/dependencies/installing-rust.html#xtensa-esp32-esp32-s2-esp32-s3
Also `cargo install espflash`

To compile and run the demo:

```sh
cargo +esp build -p printerdemo_mcu --target xtensa-esp32s2-none-elf --no-default-features --features=mcu-board-support/esp32-s2-kaluga-1 --release --config examples/mcu-board-support/esp32_s2_kaluga_1/cargo-config.toml
espflash --monitor /dev/ttyUSB1 target/xtensa-esp32s2-none-elf/release/printerdemo_mcu
```

The device needs to be connected with the two USB cables (one for power, one for data)

