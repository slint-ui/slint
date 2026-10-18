# Panel-paced touch with independent acquisition

The physical Queue and Settings drags now acquire about 133 contact coordinates per second while delivering about 58.7 movement events per second to Slint.
Each movement event carries the newest position and any preceding timestamped coordinates.
The user confirmed that both lists follow their finger and described the result as a huge upgrade, with room for improvement.

## Physical measurements

These measurements cover the first requested continuous drag on each page, with one lift at the end of each drag.
The earlier 38 Hz measurements are in `../physical-2026-10-19/analysis.json`.

| Measurement | Queue before | Queue after | Settings before | Settings after |
| --- | ---: | ---: | ---: | ---: |
| Acquisition rate | 37.95 Hz | 132.90 Hz | 37.86 Hz | 133.01 Hz |
| Median contact-coordinate gap | 26.28 ms | 7.50 ms | 26.32 ms | 7.51 ms |
| 95th percentile coordinate gap | 26.65 ms | 8.19 ms | 26.80 ms | 8.16 ms |
| Maximum coordinate gap | 33.68 ms | 14.71 ms | 52.72 ms | 14.18 ms |
| False releases during the held drag | 0 | 0 | 1 | 0 |

The before rate counts reports, including release packets; the after rate uses contact-coordinate intervals only.
That small counting difference does not account for the improvement.
There are no missing raw or dispatched trace sequence numbers in the selected drags.
The later exploratory portion of the capture has one raw trace hole and is excluded from these distributions.

| Slint movement delivery | Queue | Settings |
| --- | ---: | ---: |
| Movement rate | 58.76 Hz | 58.67 Hz |
| Median event gap | 17.08 ms | 17.06 ms |
| 95th percentile event gap | 18.20 ms | 17.90 ms |
| Maximum event gap | 19.41 ms | 26.52 ms |
| Mean newest-sample age at dispatch | 4.03 ms | 4.08 ms |
| Maximum newest-sample age at dispatch | 7.83 ms | 7.94 ms |
| Contact coordinates acquired | 657 | 800 |
| Movement events | 291 | 353 |
| Additional historical coordinates delivered | 365 | 446 |

All coordinates are accounted for: one press coordinate plus movement events plus historical coordinates equals the acquired contact-coordinate count on each page.
Each movement carries at most two historical coordinates in these drags.
The sample buffer recorded zero overflows throughout the complete capture.
Presses and releases remain responsive; pending final movement is delivered before release, even within the same panel tick.

The Settings maximum of 26.524 ms is a remaining UI dispatch outlier, between event sequences 482 and 483.
Acquisition continued independently during that interval, and the intermediate coordinates were retained.
The raw acquisition target is met in these drags, but physical UI dispatch intervals are not perfectly uniform.
Sample age at dispatch is not end-to-end touch-to-photon latency.

## Causes and changes

The false release came from treating the first lifted contact record as a release while another record in the same controller report remained active.
The driver now reads the complete report, tracks contact handover and rejects invalid acknowledgement packets.

The controller needed both the count acknowledgement and the `D000 AB` rearm command.
Adding rearm removed the approximately 38 Hz ceiling.
Fresh interrupt-edge gating then prevented repeated reads of an already acknowledged packet.

Acquisition still slowed during core 0 rendering.
Core 1 now owns touch and IMU I2C operations and checks for touch interrupt edges every 250 microseconds when its executor can run.
This is a scheduling interval, not a claim of 4 kHz physical sampling.
A bounded cross-core buffer preserves samples and contact edges.
Core 0 takes the latest movement batch once per panel tick and processes input before rendering.
A doc-hidden single-pointer event carries acquisition timestamps and history through Slint's normal pointer dispatch.
It preserves the existing pointer semantics and does not introduce easing toward the finger.

## Regression qualification

CPU remains 150 MHz, flash 75 MHz, and display SPI 60 MHz.
The UI, particle counts and visual assets are unchanged.

| Ten-second workload | Changing FPS | Mean raster time | Missed phases | Late strips | Scan errors |
| --- | ---: | ---: | ---: | ---: | ---: |
| Queue pan | 58.58 | 11.42 ms | 0 | 0 | 0 |
| Settings pan | 58.59 | 11.69 ms | 0 | 0 | 0 |
| Boosted tilted Ink | 58.68 | 5.87 ms | 0 | 0 | 0 |

The raster column excludes input dispatch, particle simulation, transfer planning and waiting.
These automated workloads demonstrate retained display cadence; they do not negate the physical dispatch outlier above.
Two complete Kiosk cycles also passed all six scene windows with zero missed phases, late strips or scan errors.
Minimum measured unused main stack was 5,972 bytes; the linked main stack is 24,716 bytes and the heap remains 310 KiB.
All 52 release host tests passed, including contact handover, history coalescing, edge preservation, overflow cancellation and direct drag behaviour with timestamped history.

Four fixed Queue and Settings frame hashes exactly match the earlier captures.
Both 76,800-pixel panel-memory checks reported zero mismatched pixels.
Disassembly and asset checks confirmed hardware floating-point instructions, expected SRAM functions and unchanged fixed artwork placement.
The verified ELF SHA-256 is `08a9e7a4b15d401a5e5a8370fd9d9b2d4d2bf29475c6133abb86af3086714930`.

`analysis.json`, `reports.csv` and `events.csv` retain the selected physical evidence.
`qualification.json`, `kiosk.json`, `captures.json` and `validation.json` retain the regression evidence.
The next performance investigation should target occasional core 0 dispatch delays while preserving the independent acquisition path.
