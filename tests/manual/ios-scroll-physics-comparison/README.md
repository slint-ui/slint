<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->

# UIKit and Slint scroll physics comparison

This manual iOS harness superimposes a translucent native `UIScrollView` over a
Slint `ScrollView`.
A passive gesture recognizer forwards the original `UITouch` objects and
`UIEvent` to Slint's underlying UIKit view.
Both implementations therefore receive the same event objects, coordinates,
and timestamps.

A live overlay shows each scroll offset, their point and percentage difference,
each frame velocity, the velocity difference, and the maximum offset separation.
The UI test target also records frame-by-frame CSV traces for speed sweeps,
boundary behavior, interruption, reversal, and rapid repeated flicks.

`testShortHardFlickRetainsReleaseMomentum` uses XCTest's public gesture API to
move 120 points at a requested 6,400 points per second.
It repeats the gesture three times and records a known expected failure when
Slint travels less than half UIKit's settled distance.

The rapid-flick helper uses private XCTest event-synthesis classes.
Its cases use 75–350 ms between lifting one touch and beginning the next.
Each timing case runs three times to expose launch-velocity instability.
Use this project only for local diagnostics.

## Setup

1. Install [XcodeGen](https://github.com/yonaskolb/XcodeGen).
2. Run `xcodegen generate` in this directory.
3. Open `NativeSlintScroll.xcodeproj` and select a development team for the app
   and UI test targets.
4. Choose an attached iPhone and run the `NativeSlintScroll` scheme in Release.

The Xcode build phase calls the repository's
`scripts/build_for_ios_with_cargo.bash` script. The Cargo dependencies use
paths relative to the repository, so the harness does not depend on any
temporary checkout location.

See [FINDINGS.md](FINDINGS.md) for the measurements collected on the original
iPhone 13 Pro Max investigation.
