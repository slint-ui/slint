# Queue and Settings Input Timing

The input phase clock removes Queue's tail-latency regression and shortens the longest Settings gaps.
The UI, particle counts, 150 MHz CPU, 75 MHz flash and 60 MHz display link remain unchanged.
Display scheduling still uses the existing hardware scan reads and safety margins.

## Root Cause

Input coalescing waited for the observed panel tick to advance.
That tick is published when the display task reads the scan position over SPI.
Transfer timing can delay that observation after the physical frame boundary.
The smaller packet layout changed this polling pattern, exposing longer input gaps even when core 0 had finished its frame work.

A captured Queue gap lasted 20.170 ms.
The preceding dispatch completed after 0.904 ms, rasterization after 11.819 ms, and publication after 13.075 ms.
The next input dispatch followed another 7.095 ms later.
This remaining delay came from waiting for the observed input phase, rather than the preceding raster or publication operation.
The temporary counters are recorded in `diagnostic-contexts.json` and are absent from the retained source.

## Change

Core 1 publishes the confirmed tick, estimated frame origin and filtered panel period as a coherent snapshot.
Input derives its current phase from that measurement and the monotonic microsecond clock.
It can predict at most one wrap beyond the latest observation.
The delivered phase cannot move backward when a later observation adjusts the estimate.
Unsigned arithmetic handles timestamp and phase-counter wrap.

This is tied to the measured panel, rather than an unrelated 60 Hz timer.
Touch positions and their timestamped history still come directly from acquisition.
The display's actual scan clock, transfer admission tests, DMA path, packet sizes and beam clearances remain unchanged.
Four regression tests cover late confirmation, measurement jitter, missing observations and counter wrap.

## Controlled Comparison

The same diagnostic firmware supported both input gates during qualification.
Switching only the gate back to observed ticks restored the long gaps.
Three recorded drags per page ran in each mode, using the Pico to schedule acquisition.
These are on-device replays of previously recorded physical contacts, not new manual finger captures.

| Same-firmware input gate | Queue maximum | Settings maximum | Gaps above 20 ms, Queue / Settings |
| --- | ---: | ---: | ---: |
| Observed tick | 20.234 ms | 20.272 ms | 9 / 3 |
| Measured phase with bounded prediction | 18.199 ms | 18.613 ms | 0 / 0 |

The runtime switch and temporary timing counters were removed before the final build.

## Retained Build

A fresh baseline and the exact retained binary each ran three drags per page.
Percentiles below pool the intervals from all three drags.

| Dispatch measurement | Queue before | Queue after | Settings before | Settings after |
| --- | ---: | ---: | ---: | ---: |
| Median gap | 17.005 ms | 17.075 ms | 17.018 ms | 17.051 ms |
| 95th percentile gap | 17.808 ms | 17.560 ms | 17.553 ms | 17.644 ms |
| 99th percentile gap | 19.949 ms | 17.963 ms | 19.050 ms | 17.925 ms |
| Maximum gap | 20.335 ms | 18.286 ms | 20.248 ms | 18.423 ms |
| Gaps above 19 ms | 25 / 871 | 0 / 870 | 12 / 1,061 | 0 / 1,056 |
| Gaps above 20 ms | 6 | 0 | 1 | 0 |
| Mean newest-sample age at dispatch | 3.994 ms | 4.018 ms | 4.224 ms | 4.249 ms |
| Maximum newest-sample age at dispatch | 9.986 ms | 9.162 ms | 12.830 ms | 10.545 ms |

Queue's maximum is also below the 19.177 ms measured before the earlier packet changes introduced the regression.
The improvement is concentrated in the tails; Settings' 95th percentile increases by 0.091 ms in this comparison.
Median intervals remain close to the approximately 17.05 ms panel period.
Dispatch still has some scheduling variation, and sample age is not end-to-end touch-to-photon latency.

All 4,371 contact coordinates are accounted for on each firmware, including coalesced history.
Each drag retains one press and one release.
There are zero sample-buffer overflows, late strips or scan errors in both replay sets.

## Regression Qualification

| Ten-second workload | Changing FPS | Mean raster | Missed phases | Late strips | Scan errors |
| --- | ---: | ---: | ---: | ---: | ---: |
| Queue pan | 58.50 | 12.036 ms | 0 | 0 | 0 |
| Settings pan | 58.57 | 12.067 ms | 0 | 0 | 0 |
| Boosted tilted Ink | 58.67 | 5.978 ms | 0 | 0 | 0 |

A complete Kiosk cycle also passes Queue, Settings and Ink with zero missed phases, late strips and scan errors.
The preceding qualification build passed two complete cycles.
All 57 release host tests pass.
The retained build has 24,604 bytes of linked main stack and 6,484 unused bytes measured in Kiosk.
The heap remains 310 KiB.

Four fixed Queue and Settings captures match the earlier RGB565 hashes exactly.
Both 76,800-pixel panel-memory checks report zero mismatches.
Code-generation checks confirm hardware floating-point instructions, expected SRAM functions and 24 unchanged artwork symbols at fixed addresses.
The retained ELF SHA-256 is `6387487ece92bb82edb39083ba5f9fbdfcb8fdf8ac1c6951a6190af958043b33`.

`before-runs.json`, `after-runs.json`, `summary.json` and `worst-gap-contexts.csv` retain the final timing evidence.
`same_binary_observed-runs.json` and `same_binary_predicted-runs.json` retain the controlled clock comparison.
`qualification.json`, `kiosk.json`, `captures.json` and `validation.json` retain the regression evidence and firmware hashes.
Use the existing `profile-touch-replay.py --repeats 3` command to repeat the retained input qualification.

The reusable lesson is to distinguish a panel boundary from the time software observes it.
Input can follow a bounded estimate of the measured phase while transfer safety continues to use hardware observations.

The [touch audit](../audit.md) records the subsequent cleanup and its exact firmware qualification.
