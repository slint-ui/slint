# Ink Particle Rendering

The vials are RGB565 Slint images with tinted glass, curved liquid surfaces, reflective bubbles, and damped sloshing.
The BOX uses 32 bubbles at full capacity, scaled by fill level.
Holding adds 30 percent to the population without changing bubble size or speed.
Extra bubbles rise from the bottom and retire at the surface after release.

`box-ink-tilt` enables fused accelerometer and gyroscope input from the BOX-3.
The display worker samples the IMU after touch processing and publishes an atomic gravity vector.
The Slint thread prepares particles once per acquired physical panel tick.
No sensor transaction runs on that thread.
Two vials prepare and paint in bounded tasks on the display core while SPI DMA runs.
The other two compose on the Slint thread.
The worker borrows plain physics state and pixel slices; all Slint image operations remain on the UI thread.
The producer joins the worker before publishing the four images.

Each vial retains two RGB565 buffers and restores previous particle footprints and moving surface rows.
The renderer caches surface shading and opaque spans.
`ink-native-assets` borrows the native 48×140 glass, liquid, mask, and meniscus artwork from flash.
Other sizes use the same procedural formulas.
The asset parity test compares the generated and procedural paths exactly.

Run the production physics and cache tests on the host:

```sh
cargo test --release \
  --manifest-path examples/mcu-board-support/esp32_s3_box_3/tests/ink-host/Cargo.toml \
  --features box-ink-particles,ink-native-assets
```

The tests cover tilt limits, sloshing, density, hold/release, clipping, resize, and retained-buffer parity.
