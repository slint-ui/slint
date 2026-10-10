<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->
<!-- cspell:ignore xcodegen xcresult XcodeGen devicectl UDID ProMotion scipy numpy matplotlib -->

# UIKit and Slint Scroll-To Comparison

This app measures how UIKit animates `setContentOffset(_:animated: true)`, so Slint's smooth `scroll-to` can be fitted to it.
A translucent UIKit `UIScrollView` lies over a Slint `Flickable` with the same geometry and content.
At a scheduled display-link callback, the app calls UIKit's `setContentOffset(_:animated: true)` and Slint's `scroll-to(…, ScrollMode.smooth)` with the same target.
It records both content offsets in every display-link callback, and every UIKit content change.

`UITests/ScrollToCaseTests.swift` has one test method per case and launches the app once per capture.
`scripts/run.sh` builds, runs the tests on a connected iPhone, copies the traces from the phone, and writes CSV files into `cases/`.
No manual steps are needed between starting the script and getting the CSV files.

## Set Up the Mac

1. Install Xcode, with an iOS SDK that supports the phone's iOS version, and open it once to accept the license.
2. Sign in to Xcode with an Apple account in a development team: **Xcode › Settings › Accounts**.
   The team ID is the 10-character identifier shown there, or on the Apple developer membership page.
3. Install [XcodeGen](https://github.com/yonaskolb/XcodeGen): `brew install xcodegen`.
4. Install Rust with [rustup](https://rustup.rs), then add the iOS target: `rustup target add aarch64-apple-ios`.
5. Install the Python packages for the plots and the fit: `python3 -m pip install matplotlib numpy scipy`.
   The CSV files don't need them.

## Set Up the iPhone

1. Connect the iPhone with a cable, unlock it, and trust the Mac.
2. Turn on **Settings › Privacy & Security › Developer Mode**, and restart when asked.
3. Turn on **Settings › Developer › Enable UI Automation**.
4. Turn off Low Power Mode, which limits ProMotion displays to 60 Hz.
5. Keep the phone unlocked during the run; the app disables auto-lock itself.

If the team is a free personal team, trust the developer certificate after the first install in **Settings › General › VPN & Device Management**, then run again.

## Run the Captures

From this folder, with your team ID:

```sh
scripts/run.sh --team ABCDE12345
```

All cases launch the app 76 times and take about 15 minutes.
To run only some cases, pass `--only` once per test method:

```sh
scripts/run.sh --team ABCDE12345 --only testCase01ScrollDownFromRest --only testCase02ScrollUpFromRest
```

The script:

1. Detects the connected iPhone; pass `--device UDID` if several are connected.
2. Generates `ScrollToComparison.xcodeproj` with XcodeGen.
   The build script compiles the Slint app of this checkout with Cargo.
3. Uninstalls the app, so no traces of an earlier run remain on the phone.
4. Runs the UI tests in Release with `xcodebuild test`.
5. Copies the app's `Documents` folder into `raw/<date-time>/Documents`.
   The `raw/` folder also gets the `xcodebuild` log and the test result bundle, and stays out of Git.
6. Runs `scripts/collect.py`, which writes the CSV files into `cases/`.
7. Runs the replay in `replay/`, which adds Slint's curves computed on the Mac.
8. Runs `scripts/fit.py`, which fits animation models to the curves.

Pass `--no-replay`, `--no-fit`, or `--no-plots` to skip steps 7 and 8, or the plots.
If the bundle identifiers `dev.slint.*` can't be registered for the team, pass `--bundle-prefix com.example`.
To collect a run again, for example after changing `collect.py`, pass `--collect-only raw/<date-time>`.

If a test fails, the script still collects the traces the phone saved, then exits with the test's status.

## Results

Commit the `cases/` folder after a run; Git ignores plots and replays, which can be regenerated.
Each case folder has a `README.md` describing the scenario.

| Case | Test method | Captures |
| --- | --- | --- |
| `01-scroll-down-from-rest` | `testCase01ScrollDownFromRest` | 10 distances from 25 to 20,000 points, 2 trials |
| `02-scroll-up-from-rest` | `testCase02ScrollUpFromRest` | 10 distances from 25 to 20,000 points, 2 trials |
| `03-scroll-to-edge` | `testCase03ScrollToEdge` | Top and bottom, from 500 and 5,000 points away, 2 trials |
| `04-retarget-same-direction` | `testCase04RetargetSameDirection` | Second target after 50 to 400 ms, 2 trials |
| `05-retarget-reverse` | `testCase05RetargetReverse` | Second target after 50 to 400 ms, 2 trials |
| `06-scroll-during-fling` | `testCase06ScrollDuringFling` | Ahead and behind, 50 to 300 ms after the release, 2 trials |

Times are seconds from the first scroll-to call; offsets are content offsets in points.
A capture's file name holds its parameters, such as `distance0400-trial1`.

### Files for Fitting

- `cases/from-rest.csv`: every sample of the cases from rest, 01 to 03, in one file.
  `uikit_progress` and `slint_progress` go from 0 at the start to 1 at the target.
  `distance_pt` is the signed scroll distance.
- `cases/from-rest-uikit-scroll.csv`: the same captures, but UIKit's offset at every content change instead of every display-link callback.
- `cases/fit-from-rest.csv`: the fitted models per capture and for all captures together, written by `scripts/fit.py`.
  `critical` is a critically damped spring with `stiffness`, for a mass of 1.
  `spring` adds the damping ratio `zeta`.
  `bezier` is a `cubic-bezier(x1, y1, x2, y2)` easing over `duration_s`.
  `t0_s` is a start delay; `rmse_pt` is the error in points.

### Files per Case

- `captures.csv`: one row per capture with its parameters, the tested engine commit and device, the start and target offsets,
  the times to reach 50, 90, and 99 percent of the distance, settling times, and the time UIKit reports the end of its animation.
  `issues` flags captures with sampling gaps above 30 ms, a list not at rest at the end, or a list that misses its target.
- `<capture>.positions.csv`: both offsets in every display-link callback.
  `uikit_presentation_offset_pt` is the offset of UIKit's presentation layer.
  `slint_tick_seconds_from_command` is the time of Slint's animation clock, which lags the callback by up to a frame.
  The progress columns are empty for captures with several scroll-to calls.
- `<capture>.uikit-scroll.csv`: UIKit's offset in every `scrollViewDidScroll` callback.
- `<capture>.events.csv`: the scroll-to calls with the offsets and velocities at that time, the fling release,
  and UIKit's `scrollViewDidEndScrollingAnimation` and `scrollViewDidEndDecelerating` callbacks.
- `<capture>.touches.csv`: the delivered touches of case 06.
- `<capture>.positions.slint.csv`: Slint's offset at the same samples, computed by the replay on the Mac.
- `plots/`: a plot per capture and an overview of the first trials, with the replay dashed.

## Replay on the Mac

The replay runs the scroll-to calls of all captures without touches through Slint's `Flickable` on the testing backend:

```sh
cd replay && cargo test --release
python3 ../scripts/collect.py --plots-only
```

Commands and samples use Slint's animation clock on the phone, so an unchanged model reproduces the phone's Slint curve.
After changing Slint's scroll-to animation, run the replay to compare the new model with UIKit, without the phone.
Set `SLINT_REPLAY_CASE=01` to replay only matching case folders.
The replay builds for the host, so it can't use code gated with `target_os = "ios"`.
Gate iOS-only animation code with `any(target_os = "ios", slint_ios_scroll_physics)`; `replay/.cargo/config.toml` sets that flag.
The replay skips case 06, whose fling would need the iOS flick physics.

## Troubleshooting

- **Signing fails:** check the team ID, or pass another `--bundle-prefix`.
  `-allowProvisioningUpdates` lets Xcode create the profiles.
- **`Unable to find a destination`:** Xcode doesn't support the phone's iOS version, or the phone isn't unlocked and trusted.
- **Tests fail with an automation error:** turn on **Enable UI Automation** in the phone's developer settings.
- **`sampling gap` in `issues`:** the display link missed frames; check Low Power Mode and thermal state, then run the case again.
- **No captures collected:** look at `raw/<date-time>/xcodebuild.log`, and check that `raw/<date-time>/Documents` has `scroll-case*.csv` files.
