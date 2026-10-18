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
