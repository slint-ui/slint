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
