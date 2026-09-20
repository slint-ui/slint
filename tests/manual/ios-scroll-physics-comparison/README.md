<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->

# UIKit and Slint scroll physics comparison

This manual iOS harness renders a native `UIScrollView` beside a Slint
`ScrollView`. Touches that begin in the native list are forwarded to Slint so
both implementations receive the same gesture. The UI test target records
frame-by-frame offsets for speed sweeps, boundary behavior, interruption,
reversal, and rapid repeated flicks.

The rapid-flick helper uses private XCTest event-synthesis classes. Use this
project only for local diagnostics.

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
