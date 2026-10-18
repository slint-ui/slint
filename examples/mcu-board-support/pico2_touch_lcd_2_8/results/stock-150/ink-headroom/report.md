# Ink Headroom Recovery

All measurements use 150 MHz CPU, 75 MHz flash, 60 MHz PIO and fixed normal landscape orientation.
Each scene warms for two seconds before a ten-second measurement window.
The display interrupt priority, DMA ownership and scan margins remain unchanged.

## Bounded Background Batches

Core 1 processes at most eight existing job steps per thread-executor poll rather than one.
Display and DMA interrupts can preempt every step, and USB and watchdog tasks retain regular thread-executor opportunities.

| Boosted Tilted Ink | Before | Batched Worker |
| --- | ---: | ---: |
| Vial preparation | 7.541–7.599 ms | 7.325–7.370 ms |
| Render and publish | 7.177–7.178 ms | 7.264–7.273 ms |
| Remaining interval | 2.286–2.337 ms | 2.409–2.442 ms |

The smaller scheduler cost recovers approximately 0.1 ms of complete-frame budget.
Nearby code placement also affects Slint raster timings, so the preparation savings are not the complete-frame gain.
Queue, Settings and both boosted Ink windows match panel cadence with zero misses, late strips and scan errors.
The worst observed transfer overhead remains 63 microseconds.
Four Queue and Settings captures match the prior qualification exactly, and both panel checks match all 76,800 pixels.
All 24 artwork resources retain their bytes and addresses; the static main-stack reservation remains 30,524 bytes.

## Trial Left Out

Specializing every shading row by bubble radius removes the inner hardware divides, as confirmed in release disassembly.
However, extra row calls leave preparation at 7.594–7.626 ms and consume another 3.8 KiB of SRAM.
The trial passes exact coverage tests but establishes no frame-budget benefit, so its implementation is discarded.

## SRAM Worker Paint Step

The core-1 paint step is inlined into a flash-resident callback in the preceding image.
Place that step in SRAM alongside the existing paint helpers, without changing its operations.
Release symbols confirm the step occupies 1,674 bytes at `0x20010bd8`.
The static main-stack reservation remains 28,748 bytes, above the unchanged 24 KiB guard.

| Boosted Tilted Ink | Batched Worker | SRAM Paint Step |
| --- | ---: | ---: |
| Vial preparation | 7.325–7.370 ms | 7.299–7.351 ms |
| Render and publish | 7.264–7.273 ms | 7.161–7.176 ms |
| Remaining interval | 2.409–2.442 ms | 2.527–2.561 ms |

Four timing windows match panel cadence with zero missed phases, late strips and scan errors.
Four Queue and Settings captures match the prior qualification, and both panel-memory checks match all 76,800 pixels.
All 42 compositor and kiosk host tests pass.

Additional profiling counters separate core-0 painting, core-1 painting and post-local join waiting.
In the instrumented control, core 0 paints for 4.916–4.951 ms and core 1 for 4.367–4.379 ms.
Join waiting is only 0.123–0.137 ms, while approximately 2.3 ms of preparation lies outside painting.
Core times include interrupts inside their measurement intervals and overlap each other; they are not additive frame costs.

Moving Red and Cyan to core 1 makes preparation worse at 8.041–8.071 ms.
That assignment is discarded; Yellow and Cyan remain on core 1.
The retained profiling counters guide further work instead of using particle counts as a cost proxy.

## RGB565 Framebuffer Hook

Ink already publishes native RGB565 images.
There is no RGB888 or RGBA expansion to remove; the display framebuffer requires a 16-bit byte swap.

An instrumented control measures approximately 5.91 ms of complete Slint raster work.
Its measured subsets include 1.03 ms of dirty-region work, 1.73 ms of texture drawing and 0.46 ms of rectangle drawing.
Text and rounded-rectangle calls are absent from settled Ink updates.
These counters do not attribute the entire raster interval.

The Pico framebuffer implements Slint's existing texture hook for opaque, unscaled, unrotated RGB565 images.
It intersects the destination with every dirty rectangle and uses Slint's packed RGB565 byte-swap copy.
Other image formats and operations retain the default renderer.
This avoids repeated generic texture setup for each image row without changing Slint scene ownership or display synchronization.

The SRAM hook occupies 962 bytes at `0x2000eeb4`.
The static main-stack reservation remains 27,652 bytes, above the unchanged 24 KiB guard.

| Boosted Tilted Ink | SRAM Paint Step | Framebuffer Hook |
| --- | ---: | ---: |
| Vial preparation | 7.299–7.351 ms | 7.226–7.248 ms |
| Slint raster | 6.050–6.057 ms | 5.523–5.524 ms |
| Render and publish | 7.161–7.176 ms | 6.730–6.775 ms |
| Remaining interval | 2.527–2.561 ms | 3.008–3.029 ms |

These are per-frame averages from repeated ten-second windows, not minimum deadline margins.
The hook recovers approximately 0.48 ms, or 19% more remaining interval, against the preceding retained build.
All normal-orientation workloads match the panel's approximately 58.7 Hz cadence with zero missed phases, late strips and scan errors.

| Workload | Previous Raster | Hook Raster | Hook FPS | Missed Phases | Late Strips |
| --- | ---: | ---: | ---: | ---: | ---: |
| Queue | 10.333 ms | 10.436 ms | 58.75 | 0 | 0 |
| Settings | 10.573 ms | 10.557 ms | 58.71 | 0 | 0 |
| Boosted tilted Ink, first window | 6.050 ms | 5.523 ms | 58.69 | 0 | 0 |
| Boosted tilted Ink, second window | 6.057 ms | 5.524 ms | 58.70 | 0 | 0 |

Queue varies by approximately 0.10 ms; Settings remains similar.
The retained optimization targets native RGB565 image drawing on Ink.
All 152 boosted bubbles retain their original sizes, colors and motion.

### Rejected Cropped-Image Trial

A separate trial puts static dry glass behind cropped dynamic image items.
All 2,054 deterministic host frames match, but the extra scene items increase Ink raster time to approximately 7.47 ms.
A temporary 312 KiB heap permits qualification; the retained build keeps the original 310 KiB heap.
The cropped-image implementation and its heap increase are discarded.

The subsequent [Ink fragmentation fix](../ink-fragmentation/report.md) releases prepared text when pages are destroyed.
Both diagnostic and clean release builds pass 24 page entries after that fix.
The framebuffer hook does not change image allocation ownership.
