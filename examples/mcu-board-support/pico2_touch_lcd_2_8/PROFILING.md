# Further Pico Renderer Profiling

The stock-speed printer runs at the measured panel cadence with maximum-size Ink boost enabled.
Further work targets lower CPU rendering time while preserving pixels, particle counts, and input behavior.

## Current Detailed Measurements

The optional `pico2-renderer-profile` image adds nested clock reads around renderer stages.
It is useful for locating work and is not the cadence qualification image.
Its boosted Ink window records one missed phase; the ordinary profiling image requires zero.
The CPU remains at 150 MHz and actual SPI at 48 MHz.

| Measured Stage | Queue | Settings | Boosted Ink |
| --- | ---: | ---: | ---: |
| Whole Slint raster | 12.924 ms | 12.752 ms | 8.386 ms |
| Dirty traversal | 2.215 ms | 1.835 ms | 1.461 ms |
| Text submission | 2.806 ms | 1.204 ms | 0.000 ms |
| Rounded rectangles | 2.682 ms | 3.118 ms | 0.614 ms |
| Texture work | 2.247 ms | 2.331 ms | 1.708 ms |
| Vial background restore | — | — | 1.295 ms |
| Particle stamping | — | — | 5.090 ms |

[Raw measurements](results/stock-150/deep-profile.json) retain call counts, cache events, and clocks.
[Instrumented firmware metadata](results/stock-150/deep-profile-firmware.json) identifies the measured binary.
Text includes nested glyph texture work; rounded rectangles are nested inside rectangle work.
These columns are overlapping timing pools, not additive savings or predictions.
Queue records 2,817 prepared-text cache hits with zero misses; Settings records 1,464 hits and one miss.
Increasing that cache is a low priority.

## Five Impactful Experiments

| Priority | Experiment | Measured Work to Target | Scope and Main Constraint |
| --- | --- | --- | --- |
| 1 | Cache rasterized text-run masks or batch glyph submission | Queue text submission: 2.806 ms | Queue and Settings; preserve native bearings, clipping, color, and exact alpha rounding |
| 2 | Cache rounded-edge coverage and border row spans | Settings rounded rectangles: 3.118 ms | Cards, controls, and chrome; key by geometry and fractional phase, and invalidate style changes |
| 3 | Reuse paint commands and accelerate dirty traversal | Queue traversal: 2.215 ms | All views; the existing geometry cache does not cache the complete paint traversal |
| 4 | Propagate dynamic-image subregion damage into Slint | Ink raster: 8.386 ms; restore: 1.295 ms | Ink currently replaces whole vial images; retain double-buffer ownership and damage both old and new bubble footprints |
| 5 | Specialize the common radius-five, full-opacity particle span | Boost stamping: 5.090 ms | Ink; hoist invariant arithmetic and compare emitted instructions before retaining any specialization |

These are hypotheses that need separate release-image timings and exact pixel comparisons.
The timing pools are ceilings on possible savings, not expected improvements.
Retained text-run masks must fit the free heap of the active page and be released before Ink allocation.
The previous highlight lookup and early highlight-clamping trials did not improve device throughput.
Do not reintroduce those approaches without new evidence.

## Tools Available Now

- The USB collector measures completed changing frames, physical panel ticks, input dispatch, packet preparation, and scan deadlines.
- `PICO_RENDERER_PROFILE=1` enables the nested timing buckets shown above.
- LLVM `objdump`, `nm`, `size`, and `addr2line` inspect emitted instructions, SRAM placement, code size, and addresses.
- Native frame capture, independent host rendering, and direct panel-memory comparison check pixels separately from throughput.
- An Arm GDB installation is present in the local STM32 toolchain.
  It requires a connected SWD debugger for this board.

## Deeper Tools Worth Adding

1. Read `DWT_CYCCNT` around selected kernels for cycle-level measurements.
   Record interrupt policy and calibrate read overhead.
2. Read XIP hit/access counters and bus-fabric performance counters around the same workloads.
   These distinguish arithmetic work, flash misses, and memory contention.
3. Configure short CoreSight trace captures into RAM through the on-chip trace FIFO and DMA.
   This needs firmware integration, decoding, bounded storage, and overflow checks.
4. Install `probe-rs` for SWD sampling with a Raspberry Pi Debug Probe or another supported probe.
   Call-stack sampling halts the CPU and perturbs display scheduling.
   Flat PCSR sampling requires verifying that the target implements the sampled register.
5. Export a bounded per-frame event trace for Perfetto or another timeline viewer.
   Collect first and transfer afterward so USB output does not dominate the measured run.

DWT, cache counters, bus counters, and internal trace capture are documented in the [RP2350 data sheet](https://datasheets.raspberrypi.com/rp2350/rp2350-datasheet.pdf).
See sections 2.1.7, 3.5.7, 3.7, and 4.4.
The [probe-rs profiling guide](https://probe.rs/docs/knowledge-base/performance-profiling/) explains sampling overhead and viewing profiles.
Its [profiling implementation](https://github.com/probe-rs/probe-rs/blob/master/probe-rs-tools/src/bin/probe-rs/cmd/profile/flat.rs) distinguishes halted and PCSR sampling.

The current firmware's USB connection provides flashing and application telemetry.
It does not provide an SWD connection to GDB or probe-rs.
Installing those tools alone does not supply the required debug wiring.
No additional profiling software was installed during this audit.
