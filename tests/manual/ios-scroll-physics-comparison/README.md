<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->
<!-- cspell:ignore xcodegen xcresult XcodeGen devicectl UDID -->

# UIKit and Slint Scroll Physics Comparison

This app overlays a translucent UIKit `UIScrollView` on a Slint `ScrollView` with the same geometry and content.
A passive gesture recognizer forwards UIKit's touches to Slint, so both lists scroll with the same delivered events.
The app records both content offsets in every display-link callback, and every delivered touch.

Each folder under `cases/` is one scroll situation, with its captured CSV files.
`UITests/ScrollCaseTests.swift` has one test method per case; it varies the case's parameters and repeats each set twice.

## Run the Captures

1. Install [XcodeGen](https://github.com/yonaskolb/XcodeGen), Rust, and the `aarch64-apple-ios` Rust target.
2. Run `xcodegen generate` in this folder.
3. Run the tests on an attached iPhone in Release, replacing the placeholders:

   ```sh
   xcodebuild test -project NativeSlintScroll.xcodeproj -scheme NativeSlintScroll \
       -configuration Release -destination "platform=iOS,id=DEVICE_UDID" \
       DEVELOPMENT_TEAM=YOUR_TEAM -parallel-testing-enabled NO \
       -only-testing:NativeSlintScrollUITests/ScrollCaseTests
   ```

   Append a method name, such as `ScrollCaseTests/testCase03ReleaseOutsideMovingOutward`, to run one case.
   All cases together launch the app about 120 times and take about 25 minutes.

The Cargo dependencies and the build script point to this checkout's engine.

## Collect the Data

Copy the app's `Documents` folder from the phone into `raw/`, which Git ignores:

```sh
xcrun devicectl device copy from --device DEVICE_UDID \
    --domain-type appDataContainer --domain-identifier dev.slint.native-scroll-prototype \
    --source Documents --destination raw
```

Then sort the captures into the case folders:

```sh
python3 scripts/collect.py raw --engine-commit TESTED_COMMIT --device "iPhone 13 Pro Max, iOS 27.0"
```

For every capture, the script writes into its case folder:

- `<parameters>-trial<n>.positions.csv`: both content offsets in every display-link callback.
- `<parameters>-trial<n>.touches.csv`: the delivered touches, coalesced samples, pan callbacks, and UIKit content offsets.
- A row in `captures.csv` with the parameters, the release state, settling times, and delivery issues.

Times are seconds from the first release; content offsets are in points, negative past the top edge.
Touch identifiers and device clocks stay out of the files.
Plots go to each case's `plots/` folder, which Git ignores; pass `--no-plots` to skip them.
