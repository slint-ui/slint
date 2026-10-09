<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->

# Scroll During a Fling

A fling at 2,000 points per second moves both lists towards larger offsets.
`delay` milliseconds after the release, a scroll-to targets 39,000 points with `reverse` 0, or 35,000 points with `reverse` 1.
The replay skips these captures, since it would need the iOS flick physics on the host.
