# Touch panning diagnosis

[The subsequent physical capture](physical-2026-10-19/report.md) measured about 38 Hz and found one false release during a held Settings drag.
The measurements below describe the earlier replay investigation.

The drag issue remains open.
These measurements distinguish pointer dispatch, rendering, display delivery, and physical touchscreen reports.
The firmware uses 150 MHz CPU, 75 MHz flash, and the existing packet display pipeline.
The UI and particles are unchanged.

## Asynchronous pointer replay

USB pointer moves arrive independently of the UI loop, unlike the panel-phased synthetic scrolling benchmark.
The gesture holds the pointer down and moves over integer positions along a sine path.
Repeated positions near its turning points deliberately produce fewer changing frames.
Consequently, its changing FPS and skipped panel phases are not a maximum-throughput or dropped-frame qualification.

| Measurement | Queue | Settings |
| --- | ---: | ---: |
| Input checks per second, averaged over the window | 394.2 | 398.1 |
| Dispatched replay events per second | 69.7 | 68.5 |
| Changing frames per second | 51.19 | 50.58 |
| Mean input arrival to raster start | 5.740 ms | 5.555 ms |
| Maximum input arrival to raster start | 15.245 ms | 16.011 ms |
| Mean input arrival to final display transfer | 37.875 ms | 36.286 ms |
| Late strips | 0 | 0 |
| Scan read errors | 0 | 0 |

The input-check averages include idle loop iterations between rendered frames.
They do not imply uniform sampling intervals or that many physical coordinates per second.
The delivery measurement ends when the final packet transfer finishes; it does not measure when pixels become optically visible.
Its cumulative counter window differs slightly from the completed ten-second raster window.
The shared maximum delivery latency was 51.069 ms since boot, rather than a separate maximum for each page.

The previous input-to-raster measurement omitted substantial display pipeline latency.
This delay can contribute to dragging feeling detached from the finger.
It does not establish the complete cause of the observed easing-like movement.

Moving dispatch before rendering did not materially improve asynchronous input-to-raster latency.
That behavior change was discarded.

## Slint drag behavior

The host regression test dispatches a continuous held drag through the real Queue and Settings UI.
It checks immediate one-to-one position changes, direction reversal, a long bottom-to-top drag, and stopping without releasing.
Both lists apply coordinates immediately without later settling in this test.
This isolates the UI property path; it does not validate physical touch reporting or display latency.

## Physical reports

The profiling firmware records successful CST328 reads, valid contacts, changed coordinates, invalid packets, I2C errors, and actual dispatched press/move/release events.
The raw ring records the six report bytes and acquisition time.
USB replay does not increment these physical counters.

Two approximately forty-second captures contained no physical contact reports.
The physical report rate and continuity of a held finger therefore remain unmeasured.
An idle reading of zero reports does not mean the controller polls at zero Hz.

Run this while physically dragging Queue or Settings:

```sh
python profile-touch.py --serial BFC13272EF67B96D --seconds 60 --output /tmp/pico-touch
```

The script records cumulative counts, rates, and raw reports without changing pages or injecting input.
Sequence gaps reveal overwritten trace entries; rates use cumulative counters and survive trace overwrites.
A valid diagnosis needs a held physical drag and its actual release packet.
The driver currently interprets any successfully read packet without a valid contact as a release.
Whether that generates false releases during a held drag remains unproven.

## Validation

- All 43 release host tests passed, including the held-drag regression.
- The diagnostic firmware built and ran on the attached device.
- Asset bytes and fixed addresses matched the qualified Ink firmware.
- Automatic USB flashing remained available.
- No direct DMA or dispatch-before-render trial code remains.
