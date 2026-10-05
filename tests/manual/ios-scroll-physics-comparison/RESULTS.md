<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->
<!-- cspell:ignore xcresult XcodeGen Murmele nondecreasing -->

# iPhone Capture Results

## Provenance

- Engine: `9effee0372d4f6044fd291be53e0d2b65bb8c0b8` on `Murmele/slint: mm/flickable-scroll-animation-v2`.
- Device: iPhone 13 Pro Max, iOS 27.0.1 (`24A446`).
- Build: Release, Xcode 27.0 (`27A266a`), iOS 27.0 SDK, Winit and Skia, Cupertino style.
- Date: October 5, 2026.
- Geometry: both viewports are `[10, 104, 408, 770]` points; both content areas are `[400, 72000]` points.
- Executable SHA-256: `1c696e42914f7cf24083bdbc9c54833fae0bcf1889e8b2e19a9acded7cb9c7ed`.

The requested 774-point viewport is clamped to the available 770 points on this device.
XcodeGen generates the missing app plist, and the scene delegate enables launch with the iOS 27 SDK.
These setup changes leave the tested engine, gesture paths, and passive touch forwarder unchanged.
Earlier launch failures produced no valid captures and aren't included in the matrix.

## Reading the Data

Each case folder contains `captures.csv` and paired position and touch CSVs for each parameter set and trial.
The files come from the README's `scripts/collect.py`, with the engine and device above recorded in every summary row.
Position curves use actual elapsed seconds and content offsets in points, without distance or time normalization.
Zero seconds is the first release; cases 8 and 9 contain two releases.
The collector's settling time means the offset remains within 0.5 points of its final recorded value.
It is a measurement convention, not UIKit's internal animation-completion signal.
A negative settling time means the final position was reached before the first release.
Such a value isn't a post-release animation duration.

The app requests 120 display-link callbacks per second; `max_sample_gap_ms` records the largest observed post-release gap.
The collector flags unequal geometry, gaps over 30 ms, and movement over 0.05 points during the final 0.2 seconds.
All captures remain in the results, including flagged traces.
Raw files and plots remain local, as specified by the README.
Device clocks, touch identifiers, and `.xcresult` bundles also remain local.

Requested speed appears in filenames and parameter columns.
Use `release_pan_velocity_y` and `finger_speed_last_25_ms` to inspect delivered behavior instead of assuming requested speed was achieved.
The latter is the collector's estimate from coalesced samples, and may disagree with UIKit's pan estimate.
The passive forwarding recognizer and per-touch logging remain part of this experiment.
This campaign has no independent UIKit-only control to isolate their effect on recognition or delivery timing.
Test success verifies app launch and synthetic-event submission, not scroll-physics parity.

For example, `01-fling-inside/speed0500-trial1` requests 500 points per second.
Its recorded release pan velocity is −2,816 points per second, while the collector's finger estimate is −40 points per second.
UIKit's release offset remains 7,200 points, while Slint's is 7,310 points.
Their final offsets are 8,600.333 and 7,553.548 points respectively.
These are observed differences in this harness; they don't establish how isolated, speed-matched native and Slint flicks compare.

## Capture Matrix

The table below includes every recorded trace.
Maximum offset difference compares simultaneous display-link samples throughout each capture, including finger-down motion.
Maximum settling-time difference compares the collector's final-position convention for the two lists.


All nine UI test methods passed, with zero failures and zero skipped tests.
The matrix contains 120 captures, 240 detailed CSVs, and nine summary CSVs.
Scenario names and expected release counts were checked against the test matrix.
All position values are finite, with timestamps in nondecreasing order.

| Case | Captures | Flagged | Maximum offset difference (pt) | Maximum settling-time difference (s) |
| --- | ---: | ---: | ---: | ---: |
| [01-fling-inside](cases/01-fling-inside/captures.csv) | 12 | 0 | 1046.785 | 0.897 |
| [02-fling-into-edge](cases/02-fling-into-edge/captures.csv) | 16 | 0 | 45.747 | 0.150 |
| [03-release-outside-moving-outward](cases/03-release-outside-moving-outward/captures.csv) | 42 | 1 | 77.810 | 0.746 |
| [04-release-outside-held](cases/04-release-outside-held/captures.csv) | 10 | 0 | 39.512 | 0.084 |
| [05-release-outside-moving-inward](cases/05-release-outside-moving-inward/captures.csv) | 6 | 0 | 1901.333 | 2.356 |
| [06-fling-near-minimum-speed](cases/06-fling-near-minimum-speed/captures.csv) | 16 | 0 | 424.000 | 2.228 |
| [07-reversal-in-overscroll](cases/07-reversal-in-overscroll/captures.csv) | 6 | 0 | 1885.667 | 3.248 |
| [08-touch-during-deceleration](cases/08-touch-during-deceleration/captures.csv) | 6 | 0 | 1258.759 | 2.787 |
| [09-touch-during-spring-back](cases/09-touch-during-spring-back/captures.csv) | 6 | 0 | 30.591 | 0.084 |

The median post-release callback interval is 8.332 ms, equivalent to about 120.0 Hz.
This describes observed callbacks; it does not guarantee that every display frame was rendered.

`03-release-outside-moving-outward/distance0100-speed1200-trial2` is flagged for `sampling gap`: its largest post-release sample gap is 33.230 ms.
The flagged capture is retained, and the table includes it.
The other 119 captures pass the collector's quality checks.
These checks don't validate gesture equivalence or establish physics parity.
