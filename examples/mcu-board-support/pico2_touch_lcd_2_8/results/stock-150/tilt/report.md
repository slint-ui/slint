# Ink Tilt and Orientation Qualification

The firmware runs at 150 MHz with 48 MHz SPI and native 320 × 240 output.
The liquid counter-rotates with device roll, capped at 30 degrees in either direction.
Its antialiased elliptical surface remains curved through zero tilt.
Bubbles follow fused true-up and slide along the glass.
Flat means upright; opposite landscape orientation requires a 400 ms dwell before the hardware flip.
Vial labels are 15 pixels tall and percentages are removed.

## Final Device Windows

Each workload settles for two seconds and retains ten seconds.
Rotation is locked during throughput measurement to separate continuous animation from full-screen orientation transitions.
The tilt workload replays accelerometer and gyro samples through the live fusion path.

| Workload | Changing FPS | Vial Composition | Slint Raster | Missed Phases | Late Strips | Scan Errors |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Normal Ink | 58.61 | 3.484 ms | 7.245 ms | 0 | 0 | 0 |
| All-vial boost | 58.59 | 5.493 ms | 7.156 ms | 0 | 0 | 0 |
| All-vial boost with tilt | 58.63 | 5.792 ms | 7.159 ms | 0 | 0 | 0 |

[Raw input windows](inputs.json) and [firmware metadata](firmware.json) retain the counters and binary identity.
These three windows establish current Ink throughput; they do not repeat the earlier full Queue, Settings, and Kiosk qualification.
Composition and raster exclude other event-loop and presentation work.

Both hardware orientations pass [controller-memory comparisons](panel.json): all 76,800 pixels match the framebuffer in each check.
The [native screenshot](../../ink-particles.png) passes host CRC validation and shows the larger labels without percentages.
Counters and controller memory do not establish optical scan quality or physical touch latency.
The device remains on Ink with live sensor input and automatic orientation enabled.

## Retained Memory and Rendering Changes

Both cores prepare their own vial physics before composing disjoint pixel buffers.
Each retained buffer restores bubble footprints and the union of its old and new surface bands.
Tilted core-1 restoration yields every two rows so display delivery can continue.
Shared meniscus and opaque-row tables save 4,928 heap bytes across four native vials.
Exact surface-cache reservation avoids vector capacity doubling.

A Settings-to-Ink stall was traced to an allocation-error panic in Rust's allocator.
Live diagnostics reported 6,308 free heap bytes, but the failed request size and largest free block were not captured.
Sharing native geometry resolved the reproduced transition failure.
Temporary panic diagnostics and square-root experiments are absent from the retained source.
Orientation changes reuse the logical framebuffer and require a complete physical upload.
Full-screen flips may span more than one panel refresh; the continuous-animation measurements exclude them.

## Deletion Audit and Validation

The audit removes unreachable sideways and upside-down bubble-spawn branches because the surface normal is capped at 30 degrees.
It also excludes IMU telemetry and flip counters from ordinary builds and removes abandoned core-assignment measurements.
Normal and profiling release configurations build without project warnings.
All 35 release host tests pass, including fusion, orientation, retained-buffer parity, two-core ownership, and bubble retirement.
A separate frozen pre-audit renderer comparison covers 9,600 frames and 64,512,000 pixels with zero differences.
The comparison exercises all four vials, complete rotations, near-zero tilt, fill changes, and hold/release cycles.
