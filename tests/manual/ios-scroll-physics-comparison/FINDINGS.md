<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->

# iOS Scroll Physics Parity Investigation

**Date:** 20–21 September 2026<br>
**Audience:** Slint runtime and input developers  
**Status:** Diagnostic prototype; candidate changes are not a production fix

## Objective

Match Slint `ScrollView`/`Flickable` behavior to a native iOS `UIScrollView`, including:

- drag displacement before release;
- the threshold between a drag and a fling;
- inertial travel and stopping time;
- interruption and reversal during deceleration;
- overscroll and spring behavior at both bounds; and
- acceleration caused by several rapid, same-direction flicks.

The comparison app displays a native UIKit list beside a Slint list on the same iPhone. A single synthesized touch path is forwarded to both views, so they receive the same gesture. A UIKit-versus-UIKit control test confirmed that the forwarding mechanism can keep two native lists synchronized.

## Test environment

| Item | Value |
|---|---|
| Device | iPhone 13 Pro Max |
| Device ID | `00008110-00022D943EB8801E` |
| Slint source branch | `mm/flickable-scroll-animation-v2` from Murmele/slint |
| Tested source snapshot | `ea8335305c237525e341ebc37df89171d3a1bd6a` |
| Build | Release |
| Native reference | `UIScrollViewDecelerationRateNormal` with bounce enabled |
| Display | ProMotion enabled in the app plist |
| Test content | 1,000 equal-height rows for long, unbounded measurements |

The harness source is in this directory. Raw traces and Xcode build products
from the original device run are not committed.

## Measurement method and limitations

The comparison app records a CSV sample on each `CADisplayLink` callback:

```text
time,phase,finger_y,uikit_offset,slint_offset,native_velocity
```

The native velocity field did not produce useful nonzero samples in the current hook. Launch velocity and decay were therefore fitted from the offset-versus-time curves. XCTest's requested gesture velocity is useful for naming test cases, but it is not a measured release velocity.

Unless otherwise stated, each numeric result below is one run on one device. The findings establish specific behavioral differences and useful candidate values; they do not establish universal constants for all iOS devices and refresh rates.

The rapid-flick test uses private XCTest event-synthesis APIs. It is appropriate for a local diagnostic harness, but it should not become a shipped dependency or a required public-API test without replacement.

## Main findings

### 1. Slint applies the movement used to recognize a drag

The existing `Flickable` behavior applies the pointer movement that crosses its 8 logical-pixel distance threshold. UIKit consumes the recognition movement and starts content movement after recognition. Because a forwarded UIKit move event can span substantially more than 8 points, this produced a nearly constant Slint lead at release.

Baseline results:

| Gesture | UIKit offset at release | Slint offset at release | Slint lead | UIKit post-release travel | Slint post-release travel | UIKit stop time | Slint stop time |
|---|---:|---:|---:|---:|---:|---:|---:|
| Fast drag | 386.333 | 417.000 | 30.667 | 210.334 | 222.489 | 1.882 s | 2.182 s |
| Slow drag | 401.000 | 417.145 | 16.145 | 0.000 | 71.942 | immediate | about 1.618 s |

Consuming the first captured movement instead of applying it to the content reduced the fast-drag release gap from 30.667 points to 0.417 points.

This correction cannot simply discard the event. A very fast gesture may contain most or all of its useful velocity information in that first captured move. Production code should separate content displacement from velocity history: consume the recognition displacement visually while retaining a correctly timestamped velocity sample.

### 2. Low-speed movement incorrectly becomes a fling

The branch starts an animation for every nonzero estimated velocity. In the slow baseline case, UIKit stopped at release while Slint traveled another 71.942 points.

An experimental iOS minimum fling velocity of 250 logical pixels per second made the tested raw XCTest velocity 200 and 400 cases stop in both views. A value of 200 was too low: the raw 400 test still gave Slint 107.5 points of post-release movement while UIKit stopped.

The value 250 is a candidate derived from this device and harness. It needs validation using measured release velocities on several devices before becoming a platform constant.

### 3. The inertial decay rate is already close; Slint's stopping tail is too long

The branch uses `DRAG = 0.135`, derived from the normal UIKit deceleration rate of approximately `0.998^1000`. Curve fits on ordinary fast gestures showed approximately 2.0 per second exponential decay for both UIKit and Slint. The central decay model is therefore close to the native reference.

The more visible difference was the end condition. Slint's iOS simulation stopped at a velocity tolerance of 1, creating a longer low-speed tail. Raising the experimental tolerance to 10 produced this result:

| Measurement | UIKit | Slint |
|---|---:|---:|
| Offset at release | 392.000 | 392.417 |
| Post-release travel | 210.000 | 203.038 |
| Time from release to stop | 1.906 s | 1.868 s |

With the candidate minimum fling threshold applied, the slow case stopped immediately in both views at offset 401.

### 4. Ordinary fling distance is within roughly five percent after the candidate fixes

The stable speed sweep below used the requested XCTest velocity as the test label. It is not a direct measurement of finger velocity at release.

| Requested velocity | UIKit post-release travel | Slint post-release travel | Slint difference |
|---:|---:|---:|---:|
| 200 | 0.0 | 0.0 | 0.0% |
| 400 | 0.0 | 0.0 | 0.0% |
| 800 | 209.7 | 220.8 | +5.3% |
| 1,600 | 525.3 | 498.7 | -5.1% |
| 3,200 | 1,273.7 | 1,228.2 | -3.6% |
| 6,400 | 3,158.7 | 3,051.1 | -3.4% |

Since the fitted decay rates agree but launch distance varies around the reference, the remaining ordinary-fling error is more consistent with release-velocity estimation and input sampling than with the decay equation.

### 5. Very fast gestures expose missing velocity samples and timestamp precision

At requested XCTest velocities of 12,800 and 25,600, UIKit accelerated strongly, while Slint moved only about 276 points in the former case and did not move in the latter.

An attempted correction seeded the tracker at pointer press and retained the first capture movement for velocity calculation. It exposed a near-zero or zero time interval, yielding an extremely large launch velocity and an animation that failed to settle. That attempt was fully reverted.

Relevant implementation properties:

- animation time uses integer milliseconds;
- recent velocity is calculated from segment time differences in milliseconds;
- the iOS launch path has no general maximum-velocity clamp; and
- `MAX_SPRING_TRANSFER_VELOCITY = 5000` only limits velocity transferred into the boundary spring, not the initial fling.

The production solution needs actual touch-event timestamps, sub-millisecond precision or explicit zero-duration handling, and preferably coalesced touch history. A native-derived maximum fling velocity should protect against invalid samples without masking normal high-speed input.

### 6. The initial lower-bound mismatch was a test geometry problem

UIKit originally stopped at 4,980 while Slint stopped at 4,992. The Material `ScrollView`'s inner `Flickable` reserves 12 logical pixels for horizontal scrollbar/padding geometry even when the policy is off, so the two effective viewports were different.

After matching the UIKit viewport to Slint's effective viewport, both lists settled at 4,992. This is a harness correction rather than a physics defect.

### 7. Boundary springs are close at moderate speeds but diverge at the highest tested speed

Direct pulls beyond the bottom after correcting the viewport:

| Requested velocity | UIKit maximum | Slint maximum | UIKit settle | Slint settle |
|---:|---:|---:|---:|---:|
| 400 | 5,162.7 | 5,156.7 | 0.62 s | 0.67 s |
| 1,600 | 5,157.7 | 5,155.4 | 0.71 s | 0.67 s |
| 3,200 | 5,167.7 | 5,149.2 | 0.80 s | 0.66 s |

Top pulls, which are unaffected by lower-bound geometry:

| Requested velocity | UIKit minimum | Slint minimum | UIKit settle | Slint settle |
|---:|---:|---:|---:|---:|
| 400 | -171.3 | -164.0 | 0.62 s | 0.66 s |
| 1,600 | -166.3 | -163.3 | 0.72 s | 0.67 s |
| 3,200 | -162.3 | -147.4 | 0.79 s | 0.67 s |

The moderate cases are close. At the highest tested speed, peak overscroll differs by about 15–19 points and Slint returns as much as 0.14 seconds sooner.

Fling-to-bound cases settled at the same corrected bound. Representative bottom-bound results:

| Drag/velocity label | Release UIKit/Slint | Final UIKit/Slint | UIKit settle | Slint settle |
|---|---:|---:|---:|---:|
| 150 / 400 | 5,101.7 / 5,104.6 | 4,992 / 4,992 | 0.59 s | 0.62 s |
| 300 / 800 | 5,041.3 / 5,040.6 | 4,992 / 4,992 | 0.63 s | 0.54 s |
| 600 / 1,600 | 4,776 / 4,776 | 4,992 / 4,992 | 0.91 s | 0.78 s |

### 8. Touch interruption works; reversal still shows launch-distance error

Touching during an active deceleration stopped both views immediately, with no movement after the stopping touch. Their offsets had already diverged because of the preceding fling.

Reversing direction during deceleration left an approximately 34-point final difference in one run, although both stopped after about 2.31 seconds. This should be retested after the velocity estimator and momentum-retention paths are corrected.

### 9. UIKit has a rapid repeated-flick acceleration mode, and Slint currently fails to reproduce it

The user's observation is correct. Apple's native list enters a much faster mode after a sufficiently rapid series of same-direction flicks.

There is public corroboration in Flutter's iOS-style physics. Flutter documents `BouncingScrollPhysics.carriedMomentum` as mimicking iOS's speed increase with repeated flings and uses this function:

```text
sign(existingVelocity) * min(0.000816 * abs(existingVelocity)^1.967, 40000)
```

References:

- [Flutter `BouncingScrollPhysics.carriedMomentum`](https://api.flutter.dev/flutter/widgets/BouncingScrollPhysics/carriedMomentum.html)
- [Flutter `ScrollPhysics.carriedMomentum`](https://api.flutter.dev/flutter/widgets/ScrollPhysics/carriedMomentum.html)

The Slint branch already contains the same formula and enables it automatically on iOS. It also has a 20 ms `MOMENTUM_RETAIN_TIMEOUT`. The presence of the formula therefore does not mean the behavior is working end to end.

Normal public XCTest drag calls were unsuitable for this test because XCTest inserted roughly 3.6–6 seconds between gestures. A diagnostic test was built with a low-level `XCSynthesizedEventRecord`:

- each flick lasts 65 ms;
- each flick contains five moves 12 ms apart;
- the next touch begins 10 ms later; and
- four flicks complete in about 290 ms.

Fitted post-release results:

| Flicks in burst | UIKit launch velocity | Slint launch velocity | UIKit decay | Slint decay | UIKit post travel | Slint post travel |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 4,341 px/s | 4,473 px/s | 2.000/s | 2.005/s | 2,142.7 | 2,229.4 |
| 2 | 4,392 px/s | 4,295 px/s | 2.005/s | 2.000/s | 2,189.0 | 2,136.7 |
| 3 | 4,240 px/s | 4,458 px/s | 2.000/s | 2.000/s | 2,120.3 | 2,221.7 |
| 4 | **8,145 px/s** | **4,458 px/s** | 2.000/s | 2.000/s | **4,071.3** | **2,221.7** |

UIKit nearly doubles launch velocity on the fourth rapid flick while preserving the same decay rate. Slint remains at its ordinary launch velocity. This isolates the defect to momentum capture, retention, or application rather than inertial decay.

The next investigation should instrument:

- the active animation velocity captured when the new touch begins;
- whether that value survives until the next release;
- the same-direction decision;
- the 20 ms retention timeout relative to actual input timestamps; and
- the final velocity before and after carried momentum is applied.

### 10. A large, very fast single swipe can hide a transient trajectory mismatch behind a close endpoint

Endpoint distance alone missed a visible difference reported during testing. A second sweep used one large drag from 90% to 10% of the screen height and requested XCTest velocities from 3,200 to 11,200 pixels per second. Four cases ended within 27 points of each other, but separated much more during the first frames after release:

| Requested velocity | Samples while dragging | Final Slint - UIKit offset | Maximum separation | Time of maximum separation |
|---:|---:|---:|---:|---:|
| 4,800 | 14 | +4.8 pt | +25.5 pt | 24 ms |
| 6,400 | 8 | +13.6 pt | +39.9 pt | 24 ms |
| 9,600 | 5 | +26.0 pt | +63.2 pt | 31 ms |
| 11,200 | 4 | +21.7 pt | +80.5 pt | 23 ms |

The 11,200 case is the clearest example. At release, Slint was 22.6 points ahead. Six milliseconds later, UIKit's recorded offset was unchanged while Slint had advanced another 56.2 points. At the following sample UIKit advanced, but the views were then separated by 74.3 points. The gap peaked at 80.5 points and gradually returned to 21.7 points.

The two views nevertheless covered almost exactly the same total post-release distance in that run: UIKit traveled 5,639.3 points and Slint traveled 5,638.4 points. This explains why an endpoint comparison said they matched while the motion was visibly out of phase.

After the initial handoff, the decay curves were close. For the 11,200 case, UIKit and Slint reached 25%, 50%, 75%, and 90% of final travel at 149/144 ms, 351/346 ms, 696/691 ms, and 1,152/1,147 ms respectively. Estimated velocities after 50, 100, 200, 400, and 800 ms were also within approximately 1.5%. Their stop times were 3.489 and 3.498 seconds.

The data therefore verifies a large transient trajectory mismatch, but it does not support a different sustained exponential deceleration curve in this particular case. The dominant defect is at the drag-to-fling handoff: Slint advances roughly one high-speed display frame before UIKit's first recorded inertial movement, then the small velocity difference slowly closes most of that lead.

A delivery-order control called UIKit's `touchesEnded` implementation before forwarding the same release to Slint. It did not remove the effect. At requested velocity 11,200, the control peaked at 75.8 points and ended 28.0 points apart, compared with 80.5 and 21.7 points in the original order. This rules out the order of those two calls as the main cause.

The problem appears only in large, fast single swipes because the harness receives very few move callbacks: four samples over 25.9 ms at 11,200 and five samples over 39.7 ms at 9,600. Investigation should focus on the release sample, coalesced touch history, and when the initial fling position is evaluated relative to the first display frame. It should retain the existing decay constant until direct instrumentation shows a later-curve error.

These measurements are model offsets sampled from one `CADisplayLink`, not a decoded framebuffer recording. CoreDevice reported that screen recording is unavailable for this attached device, so the current evidence cannot distinguish a one-frame presentation-order effect from a one-frame model update. A production fix should be checked with a ReplayKit or high-frame-rate camera recording in addition to the CSV trace.

### 11. Top-bound pull resistance is close, but the bounce-return curve changes with pull distance differently

A distance-controlled top-bound test pulled the same forwarded finger path down by 50, 100, 200, and 300 points at a requested XCTest velocity of 400 pixels per second. Each gesture then held the finger stationary for 400 ms before release. This separates resistance under the finger from the spring return after release.

| Finger pull | UIKit held exposure | Slint held exposure | Slint - UIKit | UIKit half-return | Slint half-return | UIKit settle | Slint settle |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 50 pt | 16.0 pt | 17.3 pt | +1.3 pt | 238 ms | 124 ms | 572 ms | 447 ms |
| 100 pt | 44.0 pt | 43.6 pt | -0.4 pt | 137 ms | 121 ms | 593 ms | 555 ms |
| 200 pt | 89.3 pt | 86.6 pt | -2.7 pt | 94 ms | 119 ms | 601 ms | 627 ms |
| 300 pt | 130.0 pt | 125.5 pt | -4.5 pt | 87 ms | 119 ms | 618 ms | 666 ms |

The pull resistance is close. Slint exposes slightly more background in the 50-point case and slightly less from 100 points onward. The largest measured exposure difference is 4.5 points after a 300-point finger pull.

The return curves do not match. In the 50-point case, UIKit first expands from 16 to 20 points after release before returning, while Slint returns monotonically and settles 125 ms earlier. The 100-point curves are close, with Slint settling 38 ms earlier. At 200 and 300 points, UIKit returns faster through the middle of the curve: Slint reaches half exposure 25 and 32 ms later and settles 26 and 48 ms later respectively.

Slint's normalized half-return time remains almost constant for pulls of 100 points or more: 121, 119, and 119 ms. UIKit's half-return time decreases with pull distance from 137 to 94 to 87 ms. This indicates that the two implementations scale their spring response with overscroll distance differently. Matching only the final bound and approximate total duration will not make these animations look the same.

These are single runs per pull distance on the physical iPhone 13 Pro Max. The settle time is the first sample after which exposure remains within 0.5 points of the bound. As with the fling measurements, the CSV contains model offsets sampled from `CADisplayLink`, rather than decoded screen pixels.

## Candidate changes tested separately

Some measurements used two experimental runtime changes that are not included
with this harness:

1. In `internal/core/animations/simulations/ios.rs`, increase `VELOCITY_TOLERANCE` from 1 to 10.
2. In `internal/core/items/flickable.rs`:
   - use an experimental iOS minimum fling velocity of 250;
   - do not start an inertial animation below that threshold;
   - consume the movement that first crosses the drag-recognition threshold instead of applying it to content; and
   - base recognition on total drag distance.

These changes explain the improved ordinary-drag results. They remain incomplete because consuming the capture event currently removes velocity information needed by some very fast single-event gestures.

## Relevant source and test locations

| Purpose | Location |
|---|---|
| Flickable gesture capture and momentum retention | `internal/core/items/flickable.rs` |
| Fling and carried-momentum calculation | `internal/core/items/flickable/animation.rs` |
| iOS decay and spring simulation | `internal/core/animations/simulations/ios.rs` |
| XCTest cases | `tests/manual/ios-scroll-physics-comparison/UITests/ScrollComparisonTests.swift` |
| Low-level rapid-touch synthesis helper | `tests/manual/ios-scroll-physics-comparison/UITests/DisableQuiescence.m` |

The helper filename predates its current role and should be renamed if this harness is retained.

## Recommended implementation plan

### Priority 0: correct launch velocity and repeated-fling momentum

1. Separate drag recognition from velocity sampling. The first capture movement should not move the content, but it must remain available to the velocity estimator with a trustworthy timestamp.
2. Feed platform event timestamps through the tracker. Preserve sub-millisecond resolution or explicitly reject/merge zero-duration segments. Use coalesced touch history where available.
3. Add a platform-specific maximum launch velocity based on native measurements. Treat it as protection against invalid samples, not as compensation for an inaccurate estimator.
4. Instrument and correct carried-momentum retention. Capture the active animation velocity at the new touch, retain it through the next drag, apply it only for a compatible direction, and verify the lifetime against real event timing. The current 20 ms timeout is a primary suspect.
5. Add focused unit tests for zero-time samples, a capture event containing the only meaningful movement, interruption during deceleration, same-direction repeated flicks, and opposite-direction reversal.

### Priority 1: validate platform constants and spring behavior

1. Measure the native minimum fling threshold and stop tolerance on multiple iPhones and at both 60 Hz and 120 Hz before finalizing 250 and 10.
2. Tune high-speed boundary overscroll and spring return after launch velocity is reliable.
3. Add repeatable device tests for a release-speed sweep, direct pulls at both bounds, fling exhaustion into both bounds, touch-to-stop, reversal, and rapid multi-flick bursts.

### Priority 2: broaden interaction coverage

Test short content that cannot scroll, bounce enabled and disabled, dropped or delayed frames, diagonal gestures, nested Flickables, content resize during animation, application interruption, and refresh-rate transitions.

## Suggested acceptance criteria

These tolerances are a practical starting point and should be agreed by the runtime maintainers:

- ordinary drag release offsets differ by no more than 1 logical pixel;
- Slint produces no inertia when the native view produces none;
- ordinary post-release travel differs by no more than 5%;
- ordinary stopping time differs by no more than 50 ms;
- both views settle at the same content bounds;
- high-speed overscroll peak and spring duration stay within explicitly chosen tolerances; and
- the fourth-flick launch-velocity boost ratio in the rapid-burst test matches UIKit within an agreed tolerance.

## Verification completed

- Focused iOS velocity-tracker tests: 3 passed.
- Focused iOS simulation tests: 5 passed.
- `cargo check -p i-slint-core`: passed.
- `git diff --check`: passed.
- Full `i-slint-core` suite: 365 passed and 1 failed. The failure was in the untouched `general::test_velocity_tracker_cases` case (`y only: received 666.6676 expected 266.6667`). It is outside the modified files, but it was not independently reproduced on a clean checkout, so it should not yet be classified as pre-existing.
- Rapid repeated-flick XCTest: passed and produced the four trace files described above.

The original raw trace files are not committed. The rapid trace files generated
by this harness are named `scroll-repeated-hard-flicks-1.csv` through
`scroll-repeated-hard-flicks-4.csv`.

## Conclusion

Slint's ordinary iOS deceleration curve is already close to `UIScrollViewDecelerationRateNormal`. The largest remaining problems are upstream of that curve: drag-recognition displacement, the low-speed fling threshold, velocity sampling for very fast gestures, and carried momentum across rapid repeated flicks. The rapid-burst test provides the clearest current failure: UIKit doubles its launch velocity on the fourth flick while Slint does not increase it at all, even though the branch contains the intended carried-momentum formula.
