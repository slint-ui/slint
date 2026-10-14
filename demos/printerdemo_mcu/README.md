
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

For Ink, set `BOX_BENCH_INK_HOLD` to `yellow`, `red`, `cyan`, or `black` to exercise a sustained touch hold.
Use `all` to stress all four vials simultaneously through their shared hold state.
Each held vial adds 30% more bubbles without reducing the normal population.
Additional bubbles emerge from the bottom, and release restores the normal count.
Set `BOX_BENCH_WORKLOAD=pulse` to alternate 90 panel ticks held and 30 released, including the input transitions in the measurements.
Hold runs capture both the boosted image and the restored image after release.
The BOX build uses a 32-bubble full-vial density budget; other builds retain 40.
Bubble counts scale with each fill level, and holding adds 30% to that vial.
The smaller BOX budget reserves time for touch transitions while keeping bubble size, color, and speed unchanged.
See [the hold performance report](INK-HOLD-PERFORMANCE.md) for measurements and qualification limits.

For Settings flick profiling, set `BOX_BENCH_TOUCH_X=300` to use the empty right gutter.
The default center path also exercises controls and popup transitions, so report its results separately from pure scrolling.
