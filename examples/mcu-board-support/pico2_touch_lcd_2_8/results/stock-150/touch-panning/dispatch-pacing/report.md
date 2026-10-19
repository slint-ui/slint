# Touch Dispatch During Display Publication

Settings dispatch stalls are shorter after servicing input during publication waits and using smaller display packets.
The UI, particle counts, 150 MHz CPU, 75 MHz flash and 60 MHz display link are unchanged.

## Cause

The original 26.5 ms gap was reproducible with the recorded Settings drag.
Temporary counters captured a 26.468 ms event gap containing one 25.379 ms frame operation.
Rasterization took 11.504 ms, while waiting for a free display packet took another 11.859 ms.
Core 1 continued acquiring touch samples, but core 0 could not dispatch them until publication returned.

This was a board event-loop and packet-handoff stall.
It was not an easing animation toward the finger.

## Retained Changes

The pending publication future is polled alongside a 250-microsecond input check.
Rasterization has finished before this input dispatch runs.
The published pixel buffer stays immutable, while new input updates properties for the next frame.
The existing panel-tick coalescing and timestamped history remain in place.
A host test verifies that input during the asynchronous draw callback preserves the next redraw.

Four six-strip packets replace three eight-strip packets.
Each smaller packet becomes reusable sooner.
Total pixel storage remains 92,160 bytes, and reducing rectangle capacity keeps packet metadata within the existing RAM budget.
The complete packet pool changes from 94,452 to 94,448 bytes.
Beam margins, deadline checks and the conservative transfer budget are unchanged.

## Controlled Before and After

Three recorded drags per page ran on each firmware.
Core 1 schedules the original coordinates with the Pico clock, eliminating host input-scheduling jitter.
These are replays of previously captured physical contacts, not fresh manual drags.
The original acquisition timestamps define replay timing; actual replay acquisition times accompany delivered events.
Do not touch the device during replay.

| Dispatch measurement | Queue before | Queue after | Settings before | Settings after |
| --- | ---: | ---: | ---: | ---: |
| Median gap | 17.048 ms | 17.034 ms | 17.038 ms | 17.046 ms |
| 95th percentile gap | 18.342 ms | 17.805 ms | 17.847 ms | 17.636 ms |
| 99th percentile gap | 18.730 ms | 19.961 ms | 20.892 ms | 19.093 ms |
| Maximum gap | 19.177 ms | 20.251 ms | 26.667 ms | 20.444 ms |
| Gaps above 20 ms | 0 / 870 | 9 / 872 | 16 / 1,057 | 2 / 1,059 |
| Mean newest-sample age at dispatch | 3.901 ms | 4.100 ms | 4.237 ms | 4.133 ms |
| Maximum newest-sample age at dispatch | 9.350 ms | 13.682 ms | 11.459 ms | 8.497 ms |

Settings' maximum dispatch gap decreases by 23.3%, and its count above 20 ms falls from 16 to two.
Queue improves at the 95th percentile but regresses slightly in its rarest intervals.
This is not perfectly uniform dispatch, and sample age is not touch-to-photon latency.
The subsequent [measured-phase input change](../input-phase/report.md) removes this Queue regression and shortens both lists' longest gaps.

All 4,371 contact coordinates are accounted for on each firmware, including coalesced history.
Each drag retains one press and one release, with zero sample-buffer overflows.
Both before and after replays report zero late strips and scan-read errors.

## Regression Qualification

| Ten-second workload | Changing FPS | Mean raster | Missed phases | Late strips | Scan errors |
| --- | ---: | ---: | ---: | ---: | ---: |
| Queue pan | 58.54 | 11.714 ms | 0 | 0 | 0 |
| Settings pan | 58.60 | 11.938 ms | 0 | 0 | 0 |
| Boosted tilted Ink | 58.70 | 5.867 ms | 0 | 0 | 0 |

These figures establish retained panel cadence, not lower raster CPU cost.
Two complete Kiosk cycles also pass all six scene windows with zero misses, late strips and scan errors.
All 53 release host tests pass.
The linked main stack is 24,732 bytes, with at least 6,612 bytes unused in the Kiosk run.
The heap remains 310 KiB.

Four fixed Queue and Settings frames match the earlier RGB565 hashes exactly.
Two 76,800-pixel panel-memory checks report zero mismatches.
The Settings capture was also inspected visually at native 320 by 240 resolution.
Code-generation checks confirm hardware floating-point instructions, expected SRAM functions and 24 unchanged artwork symbols at fixed addresses.

The final ELF SHA-256 is `7412c476d3f7b13a82aa14f72f8f888718b42e23ca95b07e12cb33da445a1e3c`.
`validation.json` records both firmware hashes and clock settings.
`before-runs.json`, `after-runs.json`, `summary.json` and `worst-gap-contexts.csv` retain dispatch evidence.
`qualification.json`, `kiosk.json` and `captures.json` retain regression evidence.

To repeat the drag comparison, run the profiling firmware and invoke:

```sh
python3 examples/mcu-board-support/pico2_touch_lcd_2_8/profile-touch-replay.py \
    --serial BFC13272EF67B96D --repeats 3 --output /tmp/pico-touch-replay
```

The existing `profile-printer.py` checks `poll-pan`, `settings-poll-pan` and `ink-tilt-boost` with `--require-panel-rate`.
The existing `profile-kiosk.py --cycles 2 --require-panel-rate` checks repeated page changes.

## Discarded Trials

Reducing the transfer-overhead estimate increased the number of long Settings gaps.
Waiting for a fresh panel tick after every render skipped useful input opportunities and made both pages worse.
Six smaller packets were less consistent than four for Settings.
Those code changes and temporary timing counters are absent from the final source.
