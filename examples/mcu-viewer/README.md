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
cargo +esp run --release --no-default-features --features mcu-board-support/box-optimized-display --config ../mcu-board-support/esp32_s3_box_3/cargo-config.toml
```

The BOX-3 runner selects QIO flash mode, and the optimized backend runs octal PSRAM at 80 MHz.
The LCD remains at 40 MHz.

The BOX-3 backend renders into a retained framebuffer and sends changed regions through a DMA display worker.
Large, dense changes use one bounding rectangle to avoid separate blocks appearing during the update.
Transfers follow the panel's refresh timing.
A full-screen update still sends 150 KiB over 40 MHz SPI, so it spans more than one panel refresh.
Use `mcu-board-support/esp32-s3-box-3` for the basic line-buffer backend.

Add `mcu-board-support/box-display-profile` to log rendering times, transfer times, and rectangle coordinates over the USB connection.
Leave it disabled for normal use.

The backlight stays on during display transfers.

The display pipeline is adapted from the [BOX printer backend](https://github.com/slint-ui/slint/tree/d99b59567614454aff82b94fffdbb25661d35f5f/examples/mcu-board-support/esp32_s3_box_3).
Its internal transfer packet is limited to 32 KiB to leave room for the viewer's main stack.

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
The firmware can only read designs compiled by the exact same Slint version.

To show a design on your device, follow these steps:

1. In your editor, open the live preview of a `.slint` file.
2. From the combobox in the top-right corner of the preview, select **Remote**.
3. In the list, select your device; it appears as `USB: …`.
   For the simulator, type the name of its pseudo-terminal into the manual entry field instead.

The device then displays your design and updates it after every edit.
If an edit doesn't compile, the device keeps showing the previous version.
The device's log output appears in the language server's log.

## Test the BOX Display Pipeline

From the repository root, run the packet, damage, touch, fill, and DMA tests:

```sh
rustc --edition=2024 --test examples/mcu-board-support/esp32_s3_box_3/frame_queue/host.rs -o /tmp/box-display-tests
/tmp/box-display-tests
rustc --edition=2024 --test examples/mcu-board-support/esp32_s3_box_3/tests/dma_bus.rs -o /tmp/box-dma-tests
/tmp/box-dma-tests
```

Compare the framebuffer renderer with the standard renderer across 720 frames:

```sh
cargo test --release --manifest-path examples/mcu-board-support/esp32_s3_box_3/tests/renderer-host/Cargo.toml
```

## Limitations

- Text renders only the characters that appear in the design's string literals, plus ASCII.
  At runtime, a `TextInput` can display only those characters.
- A design can't load images or fonts from files at runtime.
  The compiler embeds `@image-url` images and imported fonts when it compiles the design.
- If a design crashes the firmware, the board restarts.
  The live preview then reports the panic, and sends the design again only once you edit it.

## Measure Serial Preview Latency

Set `SLINT_MCU_PREVIEW_PROFILE` to an output file path when launching the editor or another serial-preview host.
Use the BOX firmware's `mcu-board-support/box-display-profile` feature.
Make isolated edits to a static design so each load acknowledgment corresponds to the following display batch.
The profile records source-queue time, cumulative compilation, device loading, rendering, transfer, and total elapsed time in microseconds.
The total ends at host receipt of the first transfer-completion log after the load acknowledgment.
It includes the log's return path and excludes physical LCD scanout.
The start is the updated-source message leaving the editor, rather than the operating system's mouse or keyboard event.
`host_other_us` includes coalescing, channel waiting, and superseded work before the final compilation finishes.
Device rendering and transfer durations are separate measurements; they aren't additional time to add to the total.
Leave the environment variable unset and firmware profiling disabled for normal use.
