<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Analog Clock Demo

An analog clock designed for round displays, such as smartwatch-style MCU
dev kits with a circular screen.

The window background is transparent and the face is an explicit circle, so
the UI reads as round everywhere: on round hardware the corners lie outside
the glass, and in a rectangular window (desktop, SlintPad) you see a round
clock. The face is black, so a round AMOLED only lights the pixels it needs.
The software renderer cannot rotate items, so the ticks and hands are chains
of dots placed with sin/cos.

| `.slint` Design | Rust Source | Online wasm Preview |
| --- | --- | --- |
| [`ui/clock.slint`](./ui/clock.slint) | [`rust/main.rs`](./rust/main.rs) | [Online simulation](https://slintpad.com/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/demos/analogclock/ui/clock.slint) |

Run on the desktop (simulator):

```sh
cargo run --manifest-path demos/Cargo.toml -p analogclock --release
```

To run on a microcontroller, the demo builds against the
[MCU backend](../../examples/mcu-board-support). Its README walks through
the build and flashing steps for every supported board; use those commands
with `--features=mcu-board-support/<board>` and this package, for example:

```sh
cargo build --manifest-path demos/Cargo.toml -p analogclock --no-default-features --features=mcu-board-support/pico-st7789 --target=thumbv6m-none-eabi --release
```

(The demo was developed on a smartwatch-style dev kit with a 466x466 round
AMOLED.)
