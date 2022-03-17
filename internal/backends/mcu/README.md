**NOTE**: This library is an **internal** crate of the [Slint project](https://slint-ui.com).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.

# Slint MCU backend

The MCU backend is still a work in progress.

We are currently working on getting demo running with the [Raspberry Pi Pico](https://www.raspberrypi.com/products/raspberry-pi-pico/)
and [ST7789 based screen](https://www.waveshare.com/pico-restouch-lcd-2.8.htm).
The Raspberry Pi Pico uses a RP2040 micro-controller which has 264KB of RAM and 2MB of flash memory.

The other backend is the simulator which is a way to test the software rendering backend on desktop.

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

Some environment variable must be set so the Slint compiler knows to embedd the images and font into the binary

## Run the demo:

### The simulator

```sh
WINIT_X11_SCALE_FACTOR=1 SLINT_EMBED_GLYPHS=1 SLINT_FONT_SIZES=8,11,10,12,13,14,15,16,18,20,22,24,32 SLINT_PROCESS_IMAGES=1 SLINT_STYLE=ugly cargo run -p printerdemo_mcu --features=i-slint-backend-mcu/simulator --release
```

### On the Raspberry Pi Pico

```sh
SLINT_FONT_SIZES=8,11,10,12,13,14,15,16,18,20,22,24,32 CARGO_TARGET_THUMBV6M_NONE_EABI_LINKER="flip-link" CARGO_TARGET_THUMBV6M_NONE_EABI_RUNNER="probe-run --chip RP2040" SLINT_STYLE=ugly  SLINT_PROCESS_IMAGES=1 cargo +nightly run -p printerdemo_mcu --features=mcu-pico-st7789 --target=thumbv6m-none-eabi --release
```
