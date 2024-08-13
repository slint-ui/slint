<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

### `carousel`

A custom carousel widget that can be controlled by touch, mouse and keyboard

The example can be run on desktop, wasm and mcu platforms

| `.slint` Design | Rust Source | C++ Source | Node Source | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- | --- | --- |
| [`ui.slint`](./carousel/ui/carousel_demo.slint) | [`main.rs`](./carousel/rust/main.rs) | [`main.cpp`](./carousel/cpp/main.cpp) | [`main.js`](./carousel/node/main.js) | [Online simulation](https://slint.dev/snapshots/master/demos/carousel/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/carousel/ui/carousel_demo.slint) |

![Screenshot of the Carousel Demo](https://user-images.githubusercontent.com/6715107/196679740-840a4b67-afaa-4d47-9a31-bfe643c7de48.png "Carousel Demo")

See the [MCU backend Readme](../mcu-board-support) to see how to run the example on a smaller device like the Raspberry Pi Pico.

The example can run with the mcu simulator with the following command

```cargo run -p carousel --no-default-features --features=simulator --release```
