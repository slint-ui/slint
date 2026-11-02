# Slint Viewer for Microcontrollers

See your `.slint` design on the screen of a microcontroller while you edit it on your computer.
This firmware displays whatever design your editor's live preview sends over USB.
Your editor's Slint language server compiles the design on your computer and renders images and fonts ahead of time,
so the device only runs it.

The firmware runs on the [`mcu-board-support`](../mcu-board-support) boards that provide a [host link](../mcu-board-support/README.md#host-link):

- ESP32-S3-BOX-3
- M5Stack CoreS3
- A simulator on your computer

## Run the Simulator

On Linux and macOS, you can run the firmware in a simulator.
The simulator displays designs in a window and receives them through a pseudo-terminal instead of a USB serial port.
To start the simulator, run:

```sh
cargo run
```

The simulator prints the name of its pseudo-terminal, such as `/dev/pts/3`, and displays that name in its window.

## Flash an ESP32-S3 Board

To flash an ESP32-S3 board, follow these steps:

1. Install the prerequisites listed in the [MCU board support README](../mcu-board-support/README.md#esp32):
   the ESP Rust toolchain and `espflash`.
2. Connect your board over USB-C.
3. From this directory, run the command for your board.

For the ESP32-S3-BOX-3:

```sh
cargo +esp run --release --no-default-features --features mcu-board-support/esp32-s3-box-3 --config ../mcu-board-support/esp32_s3_box_3/cargo-config.toml
```

For the M5Stack CoreS3:

```sh
cargo +esp run --release --no-default-features --features mcu-board-support/m5stack-cores3 --config ../mcu-board-support/m5stack_cores3/cargo-config.toml
```

When your board is ready, it displays `Waiting for the Slint live preview`.

**Note:** The live preview needs the serial port, but `cargo run` keeps the serial monitor open on it.
Before you connect the live preview, quit the serial monitor.

## Show a Design

**Note:** Build the Slint language server from the same checkout as the firmware,
and point your editor at it, such as with the `slint.lspBinaryPath` setting in VS Code.
The firmware can only read designs compiled from the same sources.

To show a design on your device, follow these steps:

1. In your editor, open the live preview of a `.slint` file.
2. From the combobox in the top-right corner of the preview, select **Remote**.
3. In the list, select your device; it appears as `USB: …`.
   For the simulator, type the name of its pseudo-terminal into the manual entry field instead.

The device then displays your design and updates it after every edit.
If an edit doesn't compile, the device keeps showing the previous version.
The device's log output appears in the language server's log.

## Limitations

- Text renders only the characters that appear in the design's string literals, plus ASCII.
  At runtime, a `TextInput` can display only those characters.
- A design can't load images or fonts from files at runtime.
  The compiler embeds `@image-url` images and imported fonts when it compiles the design.
- If a design crashes the firmware, the board restarts.
  The live preview then reports the panic, and sends the design again only once you edit it.
