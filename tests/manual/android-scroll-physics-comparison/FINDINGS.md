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


## Validation After Merge, 2026-09-22

The main regressions still reproduce at merge commit `096c00312f`.
The original numerical results above remain historical measurements, not measurements of this checkout.

### Environment and Method

- Device: Samsung Galaxy S21 (`SM-G991U1`), Android 15.
- Display: 1080 × 2400 physical pixels, 480 dpi (3 physical pixels per dp).
- Build: current checkout, release APK, `aarch64-linux-android`, installed before testing.
- Local manifest configuration: target SDK 33 under `package.metadata.android.sdk`.
- Ran the seven-gesture matrix twice, a native/native control, and the focused velocity-tracker unit test.
- The second matrix includes `RustStdoutStderr` in the log filter to capture `SCROLL_VELOCITY`.

This device differs from the original Galaxy A34.
The long gesture covers 376 dp here, versus 391.1 dp in the original run.
Differences between the tables cannot be attributed solely to the merge.
The shell's short gestures also vary in delivered samples and drag offsets.

### Post-Release Travel

Values below come from the second matrix, in dp.
Native travel uses the first released frame's offset as its baseline.
Slint travel uses the content offset logged by `SCROLL_VELOCITY` at release.
Both offsets settled before recording ended in every case.

| Duration | Native travel | Slint travel | Native release speed (dp/s) | Slint release speed (dp/s) |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 ms | 80.3 | 9.2 | 376.0 | 173.0 |
| 500 ms | 233.0 | 36.8 | 752.0 | 383.5 |
| 250 ms | 620.0 | 98.0 | 1,504.0 | 674.0 |
| 125 ms | 1,461.7 | 343.7 | 3,008.0 | 1,388.8 |

Slint still substantially underestimates release velocity and travels less far than native Android.
The measured distances differ from the original baseline; this cross-device run does not establish a code-level improvement.

### Short, Hard Flicks

These are settled offsets, not post-release travel.

| Trial | Native Android (dp) | Slint (dp) | Slint shortfall |
| ---: | ---: | ---: | ---: |
| 1 | 3,492.7 | 64.0 | 98.2% |
| 2 | 3,488.3 | 59.7 | 98.3% |
| 3 | 3,564.7 | 136.0 | 96.2% |

All three releases logged `SCROLL_VELOCITY,None` for Slint, versus approximately 6,000 dp/s for native Android.
Slint added zero post-release travel in these trials.
The first matrix independently reproduced a 96.9–98.2% settled-offset shortfall.

### Findings That Remain Valid or Need Qualification

- **Input history:** source inspection confirms that `track_move` still omits the segment preceding the first historical point.
  The candidate fix is absent, so its velocity and decay tables were not revalidated.
- **Native/native control:** both lists settled at 604.0 dp across 330 paired frame samples.
  Unlike the original run, 33 frames differed, with a maximum difference of 1.0 dp.
  The control supports the final-distance comparison but does not establish exact frame parity on this device.
- **Velocity-tracker test:** the same failure reproduces: `Case 'y only': Received: 666.6676, Expected: 266.6667`.
- **Release timing:** the JNI forwarding path does not emit the backend's `SCROLL_INPUT,U` diagnostic.
  This run therefore does not revalidate the original 5.3–8.8 ms interval or its timeout conclusion.
- **Decay and flywheel:** controlled-velocity decay parity and overlapping successive flings remain unverified.
  The standard matrix cannot establish either claim.
- **Top bound:** a separate downward drag at offset zero kept both offsets at zero throughout all 330 recorded frames.
- **Bottom bounds and visual edge effects:** remain unverified by this matrix.

The JNI bridge preserves historical sample ages relative to each event.
It dispatches asynchronously and does not pass the current sample's absolute event timestamp into the core input event.
The earlier event-time preservation statement should not be read as proof that Slint estimates velocity on Android's original absolute clock.

### Validation Artifacts

Local traces and the JSON summary are in `/tmp/android-scroll-validation-096c00312f/`.
The `with-diagnostics/` subdirectory contains the second matrix and `summary.json`.
`native-control.log` contains the native/native control, and `top-bound.log` contains the downward drag at the top.
`summarize.py` records how the second matrix was reduced.
These local artifacts are not committed and may be removed by temporary-directory cleanup.


## Superseded Native-Runtime Experiment, 2026-09-22

This earlier experiment fixed the input-history loss and used native Android fling motion.
The native runtime integration has since been removed in favor of the standalone implementation below.
On the same Galaxy S21, all seven final offsets are within 0.43% of native Android in the final matrix.
These measurements describe the removed experiment, not the current standalone implementation.

### Implementation

- Preserve the leading movement segment before each coalesced history batch.
- Track raw pointer movement separately from the dead zone and content clamping.
- Seed each gesture at touch-down, reset old samples, and fit a line when only two distinct timestamps exist.
- Preserve original Android sample times through a stable mapping to Slint's clock.
- Keep sub-millisecond precision in the general velocity estimator.
  The native input path retains nanoseconds; the Java comparison bridge uses nanosecond APIs on Android 14 and newer.
  Older Android versions use the bridge's millisecond fallback.
- Use `OverScroller` through the Android activity backend for non-bouncing velocity-driven flings, including the device's minimum and maximum fling velocities.
  Slint still applies dynamic content bounds and incremental offsets for virtualized lists.
  Backends without this hook retain the existing simulation, as do fixed-distance wheel animations.

The controlled native `OverScroller` probe reproduced the native widget's travel at all five tested launch speeds.
This confirmed a separate decay mismatch after correcting velocity estimation.
Using the platform implementation avoids fitting constants to one device.

The remaining short-flick discrepancy during development came from timestamp rounding.
One captured gesture moved 42 dp in 7 ms, then another 21 dp in 3.5 ms.
Rounding the latter interval to 3 ms made the quadratic fit report approximately 7,300 dp/s instead of 6,000 dp/s.
A regression test now preserves those fractional sample times.

### Final Matrix

Values are settled offsets in dp, captured after installing the release APK built from these changes.
All recorded gestures settled before the trace ended.

| Gesture | Native Android | Slint | Absolute relative difference |
| --- | ---: | ---: | ---: |
| 1,000 ms | 445.33 | 446.33 | 0.225% |
| 500 ms | 596.00 | 595.00 | 0.168% |
| 250 ms | 972.67 | 970.33 | 0.240% |
| 125 ms | 1,842.00 | 1,842.00 | 0.000% |
| 20 ms short flick, trial 1 | 3,546.67 | 3,546.67 | 0.000% |
| 20 ms short flick, trial 2 | 3,483.67 | 3,483.67 | 0.000% |
| 20 ms short flick, trial 3 | 3,586.00 | 3,570.67 | 0.428% |

The three short flicks all produced an estimated speed of approximately 6,000 dp/s and approximately 3,428.7 dp of Slint post-release travel.
Before these changes, all three logged no velocity estimate and zero post-release travel.

### Validation and Limits

- All 377 core unit tests pass, including coalesced-history, fractional-time, two-sample, delivery-delay, and native-bound regressions.
- All 30 Flickable cases, both ListView touchpad cases, and 10 ScrollView cases across five styles pass.
- The old velocity-tracker test supplied absolute positions to a delta-based API.
  It now converts positions to deltas and checks both axes; the original expected velocities pass.
- A reverse fling returns both panes to zero.
  Native Android briefly reaches -6 dp, while Slint remains clamped at zero; edge behavior is therefore not identical.
- The release APK builds and is installed on the attached Galaxy S21.
- This validates single-gesture distance alignment on this device, not exact frame-by-frame parity or parity across all Android devices.
- Native flywheel behavior across overlapping gestures, bottom-edge behavior, and visual edge effects remain outside this validation.
  Android carried-momentum policy is unchanged.

Final traces and `summary.json` are in `/tmp/android-scroll-alignment/precise-final/`.
The parent directory contains intermediate experiments, the trace summarizer, and test logs.
These artifacts are local and are not committed.


## Standalone AOSP Implementation, 2026-09-22

This is the current implementation. The native-runtime hook described above has been removed.
Velocity-driven, non-bouncing flings now run entirely in Rust in Slint core, including embedded builds.
The input-history and precise-timestamp fixes from the experiment remain.

### Implementation and Reference Validation

- Port the unbounded spline from AOSP OverScroller.java, pinned to android-15.0.0_r1.
  The attributed Apache-2.0 implementation uses a compile-time 101-entry table, explicit elapsed time,
  and no per-frame allocation or Android calls.
- Preserve dynamic limits and incremental movement for virtualized lists.
- Fix remaining-distance reporting to return the untraveled distance, and complete at the exact duration.
- Retain the existing fixed-distance wheel curve, bounce behavior, and carried-momentum policy.
- Use logical pixels (AOSP density 1), default friction 0.015, millisecond duration truncation,
  and integer total-distance truncation. Frame positions remain fractional in Slint.
- Do not implement ballistic overscroll, Android springback, native flywheel, or vendor-specific tuning.

The checked-in fixture is generated by compiling and running the unmodified pinned AOSP Java source
with a deterministic clock and minimal Android stubs. It covers both directions, zero velocity,
speeds through 12,000 logical pixels/s, two friction values, and samples before, at, and after completion.
Rust matches every reference duration and total distance exactly. Position tolerance is 0.51 logical
pixels to allow Java's frame rounding; velocity tolerance is 0.02 pixels/s plus 0.001%.
See [the reference generator documentation](../../../internal/core/animations/simulations/android/README.md)
for the source revision, licensing, regeneration command, and scope.

### Galaxy S21 Matrix

A release APK containing the standalone implementation was installed on the same SM-G991U1,
Android 15, density 3 device. All seven gestures settled before capture ended.
An initial run with the screen asleep produced no native frames and was discarded.
The matrix script now rejects captures without native comparison frames.

| Gesture | Native final offset (dp) | Slint final offset (dp) | Slint vs native |
| --- | ---: | ---: | ---: |
| 1,000 ms | 447.33 | 401.00 | -10.36% |
| 500 ms | 598.00 | 480.00 | -19.73% |
| 250 ms | 975.67 | 748.00 | -23.33% |
| 125 ms | 1832.67 | 1686.33 | -7.98% |
| 20 ms short flick, trial 1 | 3586.00 | 4502.00 | +25.54% |
| 20 ms short flick, trial 2 | 3492.67 | 4424.00 | +26.67% |
| 20 ms short flick, trial 3 | 3528.67 | 4451.00 | +26.14% |

The standalone AOSP implementation does **not** reproduce this Samsung framework's distance curve.
The previous 0.43% agreement came from the removed native-runtime integration and does not apply here.
Longer gestures finish 8–23% shorter than native, while hard flicks finish approximately 26% farther.
Short flicks now estimate approximately 6,000 dp/s, so the original missing-velocity failure remains fixed.
At that speed the standalone model travels approximately 4,360 dp after release; Samsung normally
travels approximately 3,429 dp. One native trial reported 3,444 dp between the first release frame and
settling; native release-frame accounting is not an exact measurement of the fling's starting position.

The fixed-velocity native probe was rerun with the new APK and confirmed these differences:

| Launch speed (dp/s) | Pinned AOSP / Rust distance (dp) | Samsung OverScroller distance (dp) |
| ---: | ---: | ---: |
| 376 | 35 | 80.33 |
| 752 | 118 | 233.00 |
| 1,504 | 394 | 620.00 |
| 3,008 | 1,314 | 1,461.67 |
| 6,000 | 4,360 | 3,428.67 |

These differences are measured at equal launch speeds, independently of touch input estimation.
Fixing history and adopting the stock AOSP curve therefore does not establish Samsung parity.
The former Flutter-derived curve already used essentially the same total-distance equation as AOSP;
the spline port primarily corrects the time profile and duration, rather than eliminating vendor distance differences.
No Samsung-specific constants were fitted into the portable simulation.

### Checks and Remaining Scope

- 379 core unit tests pass, including AOSP fixtures, exact completion, signed bounds, dynamic bounds,
  content-origin changes, wheel curves, and retained input/timestamp regressions.
- All 30 Flickable, two ListView touchpad, and 10 ScrollView cases across five styles pass.
  The momentum-off test now compares velocity at the same elapsed time in each fling;
  the AOSP table's small nonzero first entry affects first-millisecond finite differences.
- The core compiles for thumbv7em-none-eabihf with no default features and libm,unsafe-single-threaded.
- The release comparison APK builds, installs, and runs the seven-gesture matrix.
- The device matrix establishes settled offsets, not frame-by-frame parity, bounds/edge-effect parity,
  or parity across vendors. Ballistic overscroll, springback, and native flywheel remain deferred.

Current traces and summary.json are in /tmp/android-scroll-standalone-aosp-awake/.
These are local artifacts and may be removed by temporary-directory cleanup.


### Why a Single Friction Adjustment Cannot Close the Gap

For fixed AOSP spline constants, distance is proportional to
velocity^1.7362 / friction^0.7362.
Changing friction therefore scales all fling distances by the same factor.
The measured Samsung/AOSP distance ratio instead falls from 2.30 at 376 dp/s to 0.79 at 6,000 dp/s.
The friction values needed to match those endpoints are approximately 0.0049 and 0.0208, respectively.
Changing density or another constant scale also cannot reproduce that speed dependence.

A speed-dependent profile can reduce the distance mismatch while keeping the simulation standalone.
It requires measurements of distance, duration, and intermediate positions over a wider velocity range.
Validation must include velocities not used to fit the profile.
Matching only the five existing final distances would not establish matching animation behavior.
The current stock AOSP profile remains the reference until that additional profile is validated.


### Distance-Only Profile Feasibility

The native probe now measures 20 speeds between 50 and 12,000 dp/s.
A new capture on the same Galaxy S21 confirms the original five anchor distances.
Logarithmic interpolation between those five anchors predicts eight independently measured speeds:

| Speed (dp/s) | Native distance (dp) | Predicted distance (dp) | Error |
| ---: | ---: | ---: | ---: |
| 500 | 124.67 | 124.47 | -0.16% |
| 600 | 166.33 | 164.70 | -0.98% |
| 1,000 | 352.67 | 348.44 | -1.20% |
| 1,250 | 482.33 | 477.48 | -1.01% |
| 2,000 | 879.33 | 882.15 | +0.32% |
| 2,500 | 1163.67 | 1162.65 | -0.09% |
| 4,000 | 2038.67 | 2078.22 | +1.94% |
| 5,000 | 2697.67 | 2737.50 | +1.48% |

The largest absolute distance error is 1.95% over these eight independent speeds.
This demonstrates a feasible distance correction, not an implemented or validated animation profile.
The fit is device-specific and validated only between 376 and 6,000 dp/s, in the positive direction.
Duration, intermediate positions, reverse motion, bounds, and other devices still need validation.
Slint's runtime simulation remains the pinned AOSP model.

Reproduce this calculation with:

```sh
python3 tests/manual/android-scroll-physics-comparison/scripts/analyze-distance-profile.py /tmp/slint-dense-native-probe.log
```

The expanded probe's release APK builds and is installed on the device.
The local log and /tmp/slint-dense-native-fit.json contain the measurements and predictions.
