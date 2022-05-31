This is a "fork" of the [printer demo](../printerdemo/) modified to run on a smaller screen

# Microcontroller Setup

See [MCU setup](../../docs/mcu_setup.md) guide for extra setup steps needed for
microcontroller devices.

# Run the demo:

## The simulator

```sh
cargo run -p printerdemo_mcu --features=mcu-simulator --release
```

## On the Raspberry Pi Pico

## Using probe-run

This require [probe-run](https://github.com/knurling-rs/probe-run) (`cargo install probe-run`)
and to connect the pico via a probe (for example another pico running the probe)

Then you can simply run with `cargo run`

```sh
CARGO_TARGET_THUMBV6M_NONE_EABI_LINKER="flip-link" CARGO_TARGET_THUMBV6M_NONE_EABI_RUNNER="probe-run --chip RP2040" cargo +nightly run -p printerdemo_mcu --features=mcu-pico-st7789 --target=thumbv6m-none-eabi --release
```

## Without probe-run (Linux only!)

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

## STM32H735G-DK

Using [probe-run](https://github.com/knurling-rs/probe-run) (`cargo install probe-run`)

```sh
CARGO_TARGET_THUMBV7EM_NONE_EABIHF_RUNNER="probe-run --chip STM32H735IGKx" cargo +nightly run -p printerdemo_mcu --features=i-slint-backend-mcu/stm32h735g --target=thumbv7em-none-eabihf --release
```
