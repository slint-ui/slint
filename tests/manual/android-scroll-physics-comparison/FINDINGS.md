<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->
<!-- cspell:ignore nocapture overscroll RZCWC Scroller -->

# Android native versus Slint scroll-physics comparison

Date: 2026-09-20

## Test environment

- Device: Samsung Galaxy A34 (`RZCWC0ZJ1AB`)
- OS: Android 16
- Display: 1080 x 2340 physical pixels, density 2.8125 (450 dpi)
- Slint source: local snapshot of `mm/flickable-scroll-animation-v2` rebased on the then-current Slint `master`, commit `ea8335305c`
- Build: release, `aarch64-linux-android`
- Test UI: translucent native Android `ScrollView` over a Slint `Flickable`, with 1,000 matching 56 dp rows

## Test validity

A native/native control mode put two Android `ScrollView` instances side by side and copied each `MotionEvent` from A to B. For the 500 ms scripted swipe, both views stopped at exactly 631.1111 dp and matched frame for frame. This confirms that the event-copying harness itself does not add velocity or distance to the second view.

The updated native/Slint mode forwards the same `MotionEvent` data through a
registered JNI bridge, preserving the current sample, event time, and grouped
historical samples.
A live overlay samples both offsets on the same `Choreographer` frame.

## Short, hard flick regression

The updated matrix moves 120 dp in 20 ms and repeats the case three times.
On the Galaxy A34, the settled offsets were:

| Trial | Native Android | Slint | Slint shortfall |
|---:|---:|---:|---:|
| 1 | 3,532.1 dp | 102.8 dp | 97.1% |
| 2 | 3,491.9 dp | 62.6 dp | 98.2% |
| 3 | 3,522.5 dp | 93.2 dp | 97.4% |

All three runs reproduce the same short, high-velocity momentum loss observed
in the iOS comparison.

## Baseline result

The same 391.1 dp upward drag was sent to each pane at four durations. The table gives the distance traveled after release.

| Gesture duration | Native Android | Original Slint |
| ---: | ---: | ---: |
| 1,000 ms | 86.0 dp | 5.0 dp |
| 500 ms | 249.2 dp | 10.1 dp |
| 250 ms | 648.9 dp | 12.0 dp |
| 125 ms | 1,521.1 dp | 137.8 dp |

The `ACTION_UP` event arrived 5.3 to 8.8 ms after the last move sample, so Slint's 40 ms stopped-pointer timeout does not explain the difference.

## Concrete input-history defect

Android delivers coalesced historical points with a motion event. In `FlickableData::scroll_move`, Slint records the deltas between historical points and the delta from the last historical point to the current point. It omits the leading delta from the previous event position to the first historical point.

That missing segment makes the velocity estimate far too low. A local candidate change records this leading segment before the other historical samples.

The native Android `VelocityTracker` and Slint's estimate after that candidate change are close:

| Gesture duration | Native velocity | Slint velocity after candidate fix |
| ---: | ---: | ---: |
| 1,000 ms | 391.1 dp/s | 402.9 dp/s |
| 500 ms | 782.2 dp/s | 773.5 dp/s |
| 250 ms | 1,564.4 dp/s | 1,516.2 dp/s |
| 125 ms | 3,128.8 dp/s | 3,086.8 dp/s |

This is strong evidence that the history omission is real and that the candidate correction repairs velocity estimation for these gestures.

## Remaining decay mismatch

Even with nearly matching release velocities, Slint still travels less far after release:

| Gesture duration | Native post-release | Slint after candidate fix | Slint difference |
| ---: | ---: | ---: | ---: |
| 1,000 ms | 86.0 dp | 40.1 dp | -53.4% |
| 500 ms | 249.2 dp | 124.4 dp | -50.1% |
| 250 ms | 648.9 dp | 400.2 dp | -38.3% |
| 125 ms | 1,521.1 dp | 1,375.4 dp | -9.6% |

The candidate fix substantially improves the high-speed result, but it does not establish parity. Because the measured launch velocities now match, the remaining difference is in the fling simulation or the effective parameters used by this device's native `ScrollView`, rather than in touch velocity estimation.

Slint's `AndroidFlickParameters` predicts Slint's observed travel from its measured launch velocity, so the Slint simulation is internally consistent. The next comparison should instrument a native `OverScroller` directly with controlled input velocities and compare its duration and final distance with `AndroidFlick` across the same velocity range. That will separate Android framework behavior from Samsung `ScrollView` behavior.

## Rapid successive flings

Android's `OverScroller` enables its flywheel behavior by default, and its source adds a previous same-direction velocity to a new fling. Slint currently disables carried momentum on Android.

The shell-driven four-fling test was not precise enough to demonstrate an extra native speed boost: its final native position was approximately the one-fling result plus three drag distances. The test input needs overlapping, accurately timed pointer streams while the previous fling is still moving. The current result must not be used as proof that flywheel behavior is absent or equivalent.

## Bounds

- Pulling downward while already at the top kept both scroll offsets at zero.
- Android may still render an edge effect while its logical offset remains clamped; the captured frames did not give a conclusive visual comparison.
- Bottom-bound overscroll and rebound were not completed in this run.

## Existing test concern

The focused velocity-tracker unit test on this branch currently fails:

```text
Case 'y only': Received: 666.6676, Expected: 266.6667
```

Command:

```sh
cargo test -p i-slint-core test_velocity_tracker_cases -- --nocapture
```

This should be resolved alongside the Android history fix so that the expected estimator behavior is explicit.

## Recommended implementation order

1. Add a regression test containing a previous event plus Android-style coalesced history, and preserve the leading segment.
2. Compare `AndroidFlick` against a native `OverScroller` using identical explicit velocities, recording distance and duration.
3. Decide whether Android `Auto` should carry same-direction momentum to match `OverScroller` flywheel behavior.
4. Add deterministic tests for overlapping rapid flings and top/bottom exhaustion at several velocities.
5. Re-run the side-by-side device matrix after each physics change.

## Local artifacts

- App source: `tests/manual/android-scroll-physics-comparison`
- Runtime instrumentation: `internal/backends/android-activity` and
  `internal/core/items/flickable.rs`
- Device matrix script: `scripts/run-matrix.sh`

The raw traces and screenshots from the original physical-device run are not committed.
