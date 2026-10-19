
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
  --features mcu-board-support/esp32-s3-box-3,box-pie-fill,box-pixel-snap,box-ink-tilt
```

`box-ink-tilt` includes the touch worker, DMA backoff, native vial artwork, and the BOX-3 IMU driver.
Tilt changes the liquid surfaces and bubble trajectories.
The IMU shares I²C with touch and runs after touch processing on the display core.
The UI reads an atomic gravity vector without waiting for the bus.
Two vials compose in bounded tasks during display DMA waits, and two compose on the Slint thread.
Both finish before their Slint images are published.

Enable **Kiosk mode** at the bottom of Settings to cycle Queue, Settings, and Ink every ten seconds.
The lists move one whole pixel per acquired physical panel tick.
Touch anywhere to exit.
Add `kiosk-autostart` to the feature list to enter the loop on boot.

Buttons, switches, and combo-box backgrounds use Slint rectangles and gradients.
Opaque native RGB565 vial images use the target-buffer texture hook with exact dirty-region clipping.
Scaled, translucent, rotated, and colorized textures retain the default renderer.
See the [port qualification report](../../examples/mcu-board-support/esp32_s3_box_3/results/kiosk-tilt/README.md).

Add `box-bench` to the feature list for the optional hardware benchmark.
Set `BOX_BENCH_SCENE` to `queue`, `settings`, `ink`, or `kiosk` when building.
Each run measures three ten-second windows after warming up.
For lists, `BOX_BENCH_WORKLOAD` selects `held`, `flick`, or the recorded Queue `replay`.
Set `BOX_BENCH_WINDOWS=1` for a shorter run.
The benchmark reports packet deadlines and bounded traces, then captures the framebuffer against a canonical full redraw.
It stops injecting gestures after the capture, leaving the UI interactive.
Keep benchmark flags disabled in firmware distributed to users.

The Kiosk fixture runs the actual controller with twelve-second scenes, measuring ten seconds after each page settles.
It verifies touch-to-exit after all three scenes.
Normal Kiosk scenes remain ten seconds.
Set `BOX_BENCH_TILT=replay` to feed repeatable acceleration and angular velocity through the production fusion filter.
Real IMU transactions and touch polling continue during the replay.
Combine this with sustained or pulsed holds to exercise the maximum bubble population.

Collect each flashed run with Python and `pyserial`:

```sh
python examples/mcu-board-support/esp32_s3_box_3/collect-profile.py \
  --port PORT --output /tmp/box-profile
```

Hold fixtures emit two captures; pass `--captures 2` for those runs.
The collector checks panel deadlines, sensor errors, complete traces, and full-redraw pixel parity.
It saves native PNG captures, raw serial output, timing margins, and a JSON summary.
Use `--log /path/to/serial.log` instead of `--port` to analyze an existing recording.

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
