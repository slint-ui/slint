> **Note**
> This library is an **internal** crate of the [Slint project](https://slint-ui.com).
> This crate should **not be used directly** by applications using Slint.
> You should use the `slint` crate instead.

> **Warning**
> This crate does not follow the semver convention for versioning and can
> only be used with `version = "=x.y.z"` in Cargo.toml.

# Slint MCU backend

The MCU backend is still a work in progress.

We currently have in-tree backend for
 * the [Raspberry Pi Pico](https://www.raspberrypi.com/products/raspberry-pi-pico/)
   and [ST7789 based screen](https://www.waveshare.com/pico-restouch-lcd-2.8.htm):

   The Raspberry Pi Pico uses a RP2040 micro-controller which has 264KB of RAM and 2MB of flash memory.

 * STM32H735G-DK

 * Simulator, which is a way to test the software rendering backend on desktop.

We will make some backend API public so any board supported by rust can easily be supported

## How to use

In order to use this backend, the final program must depend on both `slint` and `i_slint_backend_mcu`.
The main.rs will look something like this

```rust,ignore
#![no_std]
#![cfg_attr(not(feature = "simulator"), no_main)]
slint::include_modules!();

#[i_slint_backend_mcu::entry]
fn main() -> ! {
    i_slint_backend_mcu::init();
    MainWindow::new().run();
    panic!("The event loop should not return");
}
```

Since i_slint_backend_mcu is at the moment an internal crate not uploaded to crates.io, you must
use the git version of slint, slint-build, and i_slint_backend_mcu

```toml
[dependencies]
slint = { git = "https://github.com/slint-ui/slint" }
i_slint_backend_mcu = { git = "https://github.com/slint-ui/slint" }
# ...
[build-dependencies]
slint-build = { git = "https://github.com/slint-ui/slint" }
```

## MCU-specific Setup

Check the [MCU Setup](../../../docs/mcu_setup.md) guide for instructions on how to install
the required tooling.

## Run the demo:

### The simulator

```sh
cargo run -p printerdemo_mcu --features=mcu-simulator --release
```

### On the Raspberry Pi Pico

#### Using probe-run

This require [probe-run](https://github.com/knurling-rs/probe-run) (`cargo install probe-run`)
and to connect the pico via a probe (for example another pico running the probe)

Then you can simply run with `cargo run`

```sh
CARGO_TARGET_THUMBV6M_NONE_EABI_LINKER="flip-link" CARGO_TARGET_THUMBV6M_NONE_EABI_RUNNER="probe-run --chip RP2040" cargo +nightly run -p printerdemo_mcu --features=mcu-pico-st7789 --target=thumbv6m-none-eabi --release
```

#### Without probe-run (Linux only!)

Build the demo with:

```sh
cargo +nightly build -p printerdemo_mcu --features=mcu-pico-st7789 --target=thumbv6m-none-eabi --release
elf2uf2-rs target/thumbv6m-none-eabi/release/printerdemo_mcu
```

Then upload the demo to the Raspberry Pi: push the white "bootsel" button on the device while connecting the
micro-usb cable to the device. This will make the Raspberry Pi expose an USB storage device to your computer.

On linux the following commands should install the demo:

```
# mount the device
udisksctl mount -b /dev/<DEVICE_OF_PICO> ## Replace with actual device name!
# upload
elf2uf2-rs -d target/thumbv6m-none-eabi/release/printerdemo_mcu
```

### STM32H735G-DK

Using [probe-run](https://github.com/knurling-rs/probe-run) (`cargo install probe-run`)

```sh
CARGO_TARGET_THUMBV7EM_NONE_EABIHF_RUNNER="probe-run --chip STM32H735IGKx" cargo +nightly run -p printerdemo_mcu --features=i-slint-backend-mcu/stm32h735g --target=thumbv7em-none-eabihf --release
```
