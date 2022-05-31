# MCU Setup

> **NOTE**: Please make sure to have Rust installed first, following the
> [building](building.md) guide!

At this time microcontroller support needs a nightly rust toolchain set up.

Please make sure this toolchain is available:

```sh
rustup toolchain install nightly
```

Working with embedded hardware in Rust becomes more convenient using
[probe-run](https://github.com/knurling-rs/probe-run). You can install this
using

```sh
cargo install probe-run
```

## Raspberry Pi Pico

### Target Support

The Raspberry Pi Pico uses an ARM-based CPU, so please make sure this target architecture is
available:

```sh
rustup target add thumbv6m-none-eabi
```

### Additional tools

The [elf2uf2-rs](https://github.com/jonil/elf2uf2-rs) tool is needed to work with the Raspberry Pi Pico.

```sh
cargo install elf2uf2-rs
```

### STM32H735G-DK

### Target Support

The STM32H735G-DK uses an ARM based CPU, so please make sure this target architecture is
available:

```sh
rustup target add thumbv7em-none-eabihf
```

### Additional tools

The STM32H735G-DK uses probe-run. See above for instructions on how to install this
