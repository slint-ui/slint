# Slint Viewer for Microcontrollers

See your `.slint` design on the screen of a microcontroller while you edit it on your computer.
This firmware shows whatever design the live preview of your editor sends it over USB.
The editor's Slint language server compiles the design on your computer,
with images and fonts rendered ahead of time, so the device only has to run it.

It runs on the boards of [`mcu-board-support`](../mcu-board-support) that provide a host link:
the ESP32-S3-BOX-3, and a simulator on your computer.

## Run the Simulator

The simulator shows the designs in a window, and takes them through a pseudo-terminal
in place of a USB serial port, on Linux and macOS:

```sh
cargo run
```

It prints the name of its pseudo-terminal, such as `/dev/pts/3`, and shows it in its window.

## Flash the ESP32-S3-BOX-3

Install the prerequisites listed in the [MCU board support README](../mcu-board-support/README.md#esp32):
the ESP Rust toolchain and `espflash`.
Then connect the box over USB-C and, from this directory, run:

```sh
cargo +esp run --release --no-default-features --features mcu-board-support/esp32-s3-box-3 --config ../mcu-board-support/esp32_s3_box_3/cargo-config.toml
```

The box shows "Waiting for the Slint live preview" once it's ready.
`cargo run` keeps the serial monitor open; quit it before you connect the live preview, which needs the port.

## Show a Design

Build the Slint language server from the same checkout as the firmware.
The unit it sends only reads back with an interpreter built from the same sources,
and it refuses a device whose firmware doesn't match.

1. Open the live preview of a `.slint` file in your editor.
2. Pick **Remote** from the combobox in the top-right corner of the preview.
3. Select the device in the list, where it appears as "USB: …".
   For the simulator, type the name of its pseudo-terminal into the manual entry instead.

The device then shows the design, and shows it again after every edit.
If an edit doesn't compile, the device keeps showing the previous version.
The device's log output appears in the language server's log.

## Limitations

- Text only renders the characters that appear in the design's string literals, plus ASCII.
  Text typed into a `TextInput` at runtime can use no others.
- A design can't load images or fonts from files at runtime; `@image-url` and font imports are
  rendered when the design is compiled.
- Every distinct image and font the box receives stays in memory until it restarts.
  The idle screen shows the free memory; press the reset button when it runs low.
- The firmware doesn't restart on its own after a crash; press the reset button.
