# Further Pico Renderer Profiling

<!-- cSpell:ignore flashinfo flashhash objcopy -->

The stock-speed printer runs at the measured panel cadence with maximum-size Ink boost enabled.
Further work targets lower CPU rendering time while preserving pixels, particle counts, and input behavior.

The [native-controls regression report](results/stock-150/native-controls-14175/report.md) compares the retained Pico firmware with PR #14175 using fresh device measurements.

The [Ink surface-motion report](results/stock-150/ink-motion/report.md) covers independent slosh, settling ripples, and subtle idle variation.

## Detailed Serial-Baseline Measurements

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

## Measured Core 1 Opportunity

The printer already sends pixel payloads through DMA channel 0.
A separate instrumented image measured core 1 activity and added RGB565 blending into a scratch buffer while pixel DMA was pending.
The CPU remained at 150 MHz and SPI at 48 MHz.
The following windows used an eight-microsecond work target checked between batches of 64 pixels.

| View | Passive DMA Await | Extra Blend Work per Frame | Blended Pixels per Frame | Changing FPS | Missed Phases / Late Strips |
| --- | ---: | ---: | ---: | ---: | ---: |
| Queue | 9.33 ms | 8.37 ms | 38,511 | 58.65 | 0 / 0 |
| Settings | 12.56 ms | 11.32 ms | 52,966 | 58.63 | 0 / 0 |
| Boosted Ink | 10.59 ms | 9.44 ms | 43,771 | 58.61 | 0 / 0 |

[Raw overlap measurements](results/stock-150/core1-overlap.json) include the passive baseline, larger work targets, and three repeated boosted-Ink windows.
All six windows with extra work and all three boost repeats meet panel cadence with zero missed phases, late strips, and scan errors.
Each boost repeat completes approximately 9.42 ms of extra work per frame.
The passive instrumented Ink baseline records one missed phase; the qualified production baseline records zero.
Instrumentation and altered code placement can perturb this tight budget.

The work target is not a hard execution limit.
Each batch finishes before the time check, and interrupts can extend a measured interval.
Observed maximum intervals reach approximately 326 microseconds across the sweep.
Cycle intervals include interrupt service within the interval, so CPU allocations are approximate.
Counts of completed pixel blends establish that substantial arithmetic and memory work happened during transfer.
Three controller-memory comparisons under background work each match all 76,800 pixels.

These results demonstrate spare compute capacity rather than a measured reduction in printer rendering time.
Queue raster work rises by approximately 0.25 ms and Settings by 0.15 ms in the smaller-work-target comparison, consistent with some added contention.
Independent Ink vial composition is now implemented with explicit buffer ownership and a join before Slint consumes the images.
The [two-core qualification](results/stock-150/core1-ink/report.md) measures the resulting wall-time reduction.
General Slint rendering also needs a design that respects its UI-thread state and partitions immutable paint work.
The experiment does not establish a twofold speedup or a safe automatic half-frame split.

The following restoration results predate the two-core vial implementation.
The temporary probe was removed from production sources after measurement.
The restored release image has byte-identical loadable segments to the qualified binary.
Queue, Settings, and boosted Ink pass the restoration check at 58.66–58.74 FPS with zero missed phases, late strips, and scan errors.

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

- The profiling USB command `flashinfo` reads the live QMI timing, read format, and read command without changing them.
  The [original flash baseline](results/stock-150/flash-clock.json) reports divisor 3 and 50 MHz quad-I/O reads at 150 MHz CPU.
  The [75 MHz qualification](results/stock-150/flash-75/report.md) measures the retained divisor-2 startup configuration and its rendering benefit.
  The [QMI timing trials](results/stock-150/qmi-timing/report.md) retain a shorter chip-select gap and uninterrupted sequential bursts.
  They also compare the rejected 133 MHz CPU configuration and the single 150 MHz flash experiment.
  SRAM accesses, cache hits, and display SPI transfers do not speed up directly with the flash clock.
- `check-printer-flash.py` compares uncached flash reads with the flashed ELF using the read-only profiling command `flashhash`.
- The USB collector measures completed changing frames, physical panel ticks, input dispatch, packet preparation, and scan deadlines.
  The coarse profile also retains stage timings for the frame with the largest total draw time, separating slow rasterization from waiting for packet space.
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
