# Touch Timing Audit

The audit covers touch acquisition, timestamped pointer history, publication waits and the measured input phase clock.
The UI, particles, 150 MHz CPU, 75 MHz flash and 60 MHz display link remain unchanged.

## Deletion Audit

Removed the single-use public touch-buffer wrapper and its unused `Batch` import.
Kept the queue operation beside its only caller.
Removed a stale investigation recommendation and the implementation hash invalidated by history cleanup.
Clarified that the original physical capture predates the retained contact fixes.
Its original source revision remains in the immutable capture metadata.

No temporary timing counters, input-clock switches, alternate render gates or rejected packet layouts remain in the source.
Kept the feature-gated acquisition trace and device-timed replay because they reproduce the measured input problems.
Kept unique before-and-after evidence, rather than deleting measurements that explain the retained design.
Stored the new dispatch events once as CSV instead of verbose JSON.

## Simplification Audit

Replaced bounded division with a threshold comparison when predicting one panel boundary.
Read the input phase before locking the sample buffer, keeping cross-core clock synchronization outside that buffer's critical section.
Read the window scale factor once for the current event and its history.

Kept the coherent cross-core snapshot and monotonic phase frontier.
They prevent inconsistent clock reads and duplicate phases after measurement jitter.
Kept one-frame prediction, physical edge handling, bounded history and overflow cancellation.
Kept the publication input pump, four display packets and all hardware scan safety checks.
Removing these would undo the measured latency or stability fixes.

## Exact Retained Build Qualification

Three device-timed replays of each previously captured drag ran on the audited binary.
These are recorded physical gestures, not fresh manual finger tests.

| Dispatch gap | Queue | Settings |
| --- | ---: | ---: |
| Median | 17.038 ms | 17.061 ms |
| 95th percentile | 17.661 ms | 17.577 ms |
| 99th percentile | 17.876 ms | 17.877 ms |
| Maximum | 18.120 ms | 18.238 ms |
| Gaps over 19 ms | 0 / 868 | 0 / 1,056 |

All 4,371 contact coordinates are accounted for, including coalesced history.
Each drag has exactly one press and release.
The replays have zero buffer overflows, late strips and scan errors.
The preceding retained build measured 18.286 ms and 18.423 ms maximum gaps.
These small differences do not establish a separate performance gain from cleanup.

| Ten-second workload | Changing FPS | Mean raster | Missed phases | Late strips | Scan errors |
| --- | ---: | ---: | ---: | ---: | ---: |
| Queue pan | 58.50 | 11.282 ms | 0 | 0 | 0 |
| Settings pan | 58.58 | 11.619 ms | 0 | 0 | 0 |
| Boosted tilted Ink | 58.57 | 5.751 ms | 0 | 0 | 0 |

A complete Kiosk cycle passes all three views with zero missed phases, late strips and scan errors.
All 57 release host tests pass.
Four fixed captures remain pixel-identical, and two 76,800-pixel panel-memory comparisons have zero mismatches.
The Queue capture was also inspected at native resolution.
Code-generation checks retain hardware floating-point instructions, expected SRAM functions and 24 unchanged artwork symbols at fixed addresses.
The linked main stack remains 24,604 bytes, with at least 5,804 unused bytes measured during Kiosk.
The heap remains 310 KiB.

The exact qualified ELF SHA-256 is `b49909dd4e049d2160c4d814fda582e554b6d9a87ccc45d607513290e5083f73`.
The post-flash clock read confirms 150 MHz CPU and 75 MHz flash.
[audit/validation.json](audit/validation.json) records artifact identity and validation checks.
[audit/events.csv](audit/events.csv), [audit/runs.json](audit/runs.json) and [audit/summary.json](audit/summary.json) retain the dispatch evidence.
[audit/qualification.json](audit/qualification.json), [audit/kiosk.json](audit/kiosk.json) and [audit/captures.json](audit/captures.json) retain hardware qualification.

## History Cleanup

The 14 recent touch commits and this cleanup are consolidated into seven focused commits:

1. Physical touch profiling and baseline evidence.
2. Contact handover, acknowledgement and fresh interrupt reads.
3. Timestamped single-pointer events in Slint.
4. Core 1 acquisition and panel-paced movement history.
5. Device-clock replay of captured gestures.
6. Responsive publication waits and four smaller display packets.
7. Measured-phase input timing, cleanup and final qualification.

Earlier board and renderer history is outside this cleanup.
Historical firmware hashes identify the measured artifacts; rebuilding after Git history changes can produce different binary metadata.
The rewritten history preserves the final source tree exactly.
