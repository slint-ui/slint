
This is a "fork" of the [printer demo](../printerdemo/) modified to run on a smaller screen.

See the [MCU backend Readme](../../examples/mcu-board-support) for more info.

Original demo was designed by Matthias Grimme.

## Qualified ESP32-S3-BOX-3 build

The optimized BOX-3 build uses native 320×240 rendering, pixel-snapped lists, and animated ink vials.
It synchronizes immutable DMA packets to the panel at approximately 59.9 Hz.
The panel SPI clock is 40 MHz; the flash clock is a separate setting.

Build with the Espressif Rust toolchain and hardware floating point enabled:

```sh
RUSTFLAGS="-C link-arg=-nostartfiles -C link-arg=-Wl,-Tlinkall.x -C target-feature=+fp" \
  cargo +esp build --release --no-default-features \
  --manifest-path demos/printerdemo_mcu/Cargo.toml \
  --config examples/mcu-board-support/esp32_s3_box_3/cargo-config.toml \
  --features mcu-board-support/esp32-s3-box-3,box-pie-fill,box-pixel-snap,box-ink-particles,mcu-board-support/box-dma-backoff,mcu-board-support/box-touch-worker
```

Add `box-bench` to the feature list for the optional hardware benchmark.
Set `BOX_BENCH_SCENE` to `queue`, `settings`, or `ink` when building.
Each run measures three ten-second windows after warming up.
For lists, `BOX_BENCH_WORKLOAD` selects `held`, `flick`, or the recorded Queue `replay`.
Set `BOX_BENCH_WINDOWS=1` for a shorter run.
The benchmark reports packet deadlines and bounded traces, then captures the framebuffer against a canonical full redraw.
It stops injecting gestures after the capture, leaving the UI interactive.
Keep benchmark flags disabled in firmware distributed to users.

For Settings flick profiling, set `BOX_BENCH_TOUCH_X=300` to use the empty right gutter.
The default center path also exercises controls and popup transitions, so report its results separately from pure scrolling.
