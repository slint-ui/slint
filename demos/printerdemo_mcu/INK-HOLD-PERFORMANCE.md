# Ink Hold Performance on ESP32-S3-BOX-3

Measured on October 14, 2026, at native 320×240 resolution, CPU 240 MHz, panel SPI 40 MHz, and hardware floating point enabled.
The display pipeline uses its calibrated panel cadence, approximately 59.9 Hz.
See [the release build flags](README.md#qualified-esp32-s3-box-3-build).

## Behavior and Particle Budget

Hold a vial to emit 30% more bubbles from its bottom.
Births are staggered six panel ticks apart, and release removes the additional bubbles.
Existing bubbles keep their positions and velocities through both transitions.
Leaving the page clears its hold state.

The BOX full-vial density budget is 32 bubbles; simulator builds retain 40.
Counts scale with the fill level, with a partially faded final bubble representing fractional counts.
Bubble sizes, colors, rising speeds, glass, and lighting retain their existing formulas.
The current four fill levels produce these integer draw counts:

| Vial | Fill | Normal | Held |
| --- | ---: | ---: | ---: |
| Yellow | 40% | 13 | 17 |
| Red | 65% | 21 | 28 |
| Cyan | 50% | 16 | 21 |
| Black | 80% | 26 | 34 |
| Total | | 76 | 100 |

## Why the BOX Budget Is Smaller

The original full-vial budget of 40 passed sustained holds for all four vials, including simultaneous load.
It did not meet every deadline during real Slint press/release transitions.
The Black pulse test repeated 16 panel frames and recorded 30 late packets across three ten-second windows.
Touch dispatch cost approximately 1.4–1.8 ms at those transitions.
A late packet then made the next producer wait for the previous immutable DMA packet, amplifying the visible stall.
The measured particle generator averaged about 3.5 ms, with no image copy-on-write copies during the probe.

Exact-pixel shader, background-restore, input-order, layout, and profiling-hash trials did not eliminate the transition misses.
Those trials were removed.
Reducing the BOX density budget by 20% reserves time for input without changing the remaining bubbles' rendering or motion.

## Final Device Measurements

Each row contains three ten-second windows, after a one-second warmup per window and three seconds of startup.
The pulse fixture holds for 90 panel ticks and releases for 30, including both transitions inside the measured windows.
Individual vial runs dispatch Slint touch events at their actual hit areas and assert the resulting hold state.
The simultaneous cases set all four hold flags directly; they stress rendering, rather than emulate four physical fingers.
The touch worker polls throughout the measured runs and reported no read errors.

Draw time includes rasterization and publication, but excludes particle generation and touch dispatch.
Producer time spans frame acquisition through the start of publication, including those earlier tasks.
The minimum margin measures packet readiness before the intended panel blank and therefore also accounts for packing.

| Workload | Frames | FPS | Mean Draw (ms) | Mean Producer (ms) | Minimum Ready Margin (ms) | Misses |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Yellow | 1798 | 59.943 | 11.342 | 13.649 | 0.436 | 0 |
| Red | 1798 | 59.946 | 11.260 | 13.626 | 0.428 | 0 |
| Cyan | 1799 | 59.945 | 11.306 | 13.670 | 0.339 | 0 |
| Black | 1798 | 59.949 | 11.261 | 13.633 | 0.398 | 0 |
| All four (synthetic) | 1799 | 59.952 | 11.251 | 13.907 | 1.340 | 0 |
| All four (synthetic) sustained | 1798 | 59.955 | 11.239 | 14.196 | 1.260 | 0 |

Every window recorded zero repeated panel frames, skipped panel cycles, timeouts, late packets, late cycles, or truncated traces.
The smallest measured margin is narrow; rerun pulse and sustained-load fixtures after changes to the Ink renderer or input processing.
These finite tests qualify the measured workloads; they do not guarantee every future interaction or verify the physical panel optically.

## Pixel and Interaction Checks

Every device run captured a boosted frame and a restored frame after release.
Each 76,800-pixel capture matched a canonical full redraw with zero differences.
Native validation compared 720 frames per page for Queue, Settings, and Ink against the alternative framebuffer and damage paths.
Queue and Settings retained their previous golden digests.
Ink's digest changed with its deliberate density reduction.

Eight particle tests passed for both BOX and simulator populations.
They cover density, motion, bottom-up births, hold/release continuity, liquid clipping, and both cached buffers.
Native interaction checks exercise all four touch paths, release outside the vial, simultaneous holds, navigation cleanup, and reentry.

## Reproduce

Add `box-bench` to the documented release feature list.
For each of `yellow`, `red`, `cyan`, `black`, and `all`, rebuild with:

```sh
BOX_BENCH_SCENE=ink BOX_BENCH_WINDOWS=3 \
BOX_BENCH_INK_HOLD=black BOX_BENCH_WORKLOAD=pulse \
  <release build command>
```

Repeat `all` with `BOX_BENCH_WORKLOAD=held` for sustained maximum load.
Flash each application at `0x10000`, reset the device, and collect serial output through both `PRINTER_CAPTURE_END` markers.
Keep the panel untouched during injected gestures.
Check the per-window deadline counters, capture parity, and bounded pipeline traces.
Disable `box-bench` in the distributed interactive build.
