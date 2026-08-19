# Safe UI on the ESP32-S3-BOX-3

A bare-metal port of the [safe-ui](../) example to the
[ESP32-S3-BOX-3](https://github.com/espressif/esp-box), on
[esp-hal](https://docs.espressif.com/projects/rust/esp-hal/latest/) and
[embassy](https://embassy.dev). No ESP-IDF, no `std`.

It reuses [`app/`](../app) unchanged: the same `main.slint` scene and the same
`app_main` event loop the desktop backend drives. Only the `Platform`
implementation is board specific.

## What the backend provides

- **Clock** - `embassy_time::Instant`, whose time driver comes from the esp-rtos
  scheduler started in `main`.
- **Display** - the 320x240 ILI9342C panel on SPI2, through `mipidsi`. The scene
  is sized for exactly this panel, so the backend renders it into a PSRAM frame
  buffer at its native size and converts the RGB8 pixels to the panel's RGB565.
- **Touch** - the GT911 controller on I2C0, read when it says it has something
  to report. Its interrupt line doubles as the pin that selects the I2C address
  at reset, so `board.rs` drives it for the reset sequence and then hands it back
  to the controller as an input.
- **Waiting** - whichever comes first of that interrupt and an
  `embassy_time::Timer` for the next color step, so nothing polls. If the
  interrupt never arrives, touches are still picked up at the color step's
  once-a-second wake rather than lost.

## Prerequisites

- The esp-rs Rust fork and the Xtensa toolchain, installed with
  [`espup`](https://github.com/esp-rs/espup):

  ```
  cargo install espup espflash
  espup install
  ```

- The Xtensa GCC linker on `PATH`. `espup`'s export script sets it up, and so
  does ESP-IDF's:

  ```
  . ~/export-esp.sh
  ```

- The Slint SC compiler, built once into the shared target directory. It runs on
  the host, and the app's `build.rs` looks for it under the profile the app is
  built with, so use `--release` to go with the `cargo run --release` below:

  ```
  cargo build -p slint-compiler --no-default-features --features slint-sc --release
  ```

  Override `SLINT_COMPILER` to use a binary from somewhere else.

## Building and running

From this directory, with the board attached over USB-C:

```
cargo run --release
```

`.cargo/config.toml` selects the `xtensa-esp32s3-none-elf` target and
`espflash` as the runner, so this flashes the board and opens the serial
monitor. `cargo build --release` alone produces the ELF at
`<repo>/target/xtensa-esp32s3-none-elf/release/slint-safeui-esp32-s3-box`.

Run cargo from this directory, not with `--manifest-path` from elsewhere:
cargo looks for `.cargo/config.toml` from the working directory, so from
anywhere else the target, the runner and `SLINT_COMPILER` don't apply and the
build goes to the host.

The crate is a member of the [safe-ui workspace](../Cargo.toml) but not one of
its `default-members`, so it stays out of host builds run from there.
