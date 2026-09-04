<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->

# Public Transit Airlock Review Demo

This 320×240 image-only UI is a proposal for review and feedback.
It does not replace `../app` or participate in its Cargo, CMake, desktop, or firmware builds.
It contains Slint source and shared PNG assets, with no application Rust code.

This is not a safety implementation or an SC-compatible build target.
The reusable components bind custom inputs, which the current SC compiler rejects.
The standalone simulation also uses a global, timer, and keyboard handling that require ordinary Slint.
No SC compiler changes are included.

## Run the Demo

From the repository root:

```sh
cargo run -p slint-viewer --no-default-features --features backend-winit,renderer-software -- examples/safe-ui/app2/demo.slint
```

With `slint-viewer` installed:

```sh
slint-viewer examples/safe-ui/app2/demo.slint
```

Tap anywhere to dismiss the startup title overlay; this first tap does not operate the demo.
The startup overlay combines a plain 60% white rectangle and a 48-pixel title PNG without moving any content.
The rectangle is demo-only; the host-facing screen remains image-only.
It stays hidden for the rest of the session, including after a reset.
Select ENTER to start six seconds of securing, followed by nine seconds of equalizing.
Select EXIT OUTER to reach Complete, then EXIT INNER to return to Ready.
Double-click or double-tap the occupant chamber, or press F, to simulate Fault.
Select the red emergency banner, or press R, to reset the simulation.
Fault injection and reset gestures belong only to the demo.

## Structure

- `main.slint`: one screen with six `TransitView` states, not six separate view components.
- `demo.slint`: `DemoController` drives the screen inputs and handles its action callbacks.
- `components.slint`: image components, including the `DoorState` API and single-digit `RasterDigit`.
- `progress.slint`: ten-segment percentage indicator, clamped to 0–100; 65% lights six segments.
- `progress-text.slint`: two-digit countdown using two `RasterDigit` instances, clamped to 00–99.
- `assets/`: shared PNG artwork; digit and progress assets have separate subdirectories.

The host-facing screen exposes `view`, `seconds-remaining`, and `progress-percent` inputs.
Its parameterless callbacks are `enter-requested`, `exit-outer-requested`, and `exit-inner-requested`.
Slint bindings update the existing elements when these inputs change.
The screen does not make door-control or safety decisions.

## Review Focus

- Screen content, legibility, and state feedback at 320×240.
- Component boundaries and the host-facing API.
- PNG reuse and suitability for a future SC integration.

Replacing `app`, wiring a firmware host, and making the UI SC-compatible are separate work.

## Artwork

The PNGs were exported at native size from the Public Transit concept in Figma.
The source file is `AwH2AA7IrUYRfIdmGSN1YT`, export section `2086`.
Text is baked into the artwork; no runtime fonts or vector assets are required.
The demo title uses `logo/slint-logo-simple-light.svg` from this repository, rasterized in Figma.
The title is a native-size PNG used only by `demo.slint`.
The fault background combines the original background, pressure warning at (118,115), and emergency panel at (7,181).
Those three Figma exports are composited at native size with the Slint software renderer.
The eleven `progress/progress-*.png` states combine the Figma base and segment overlays at their native pixel offsets.
One image selects the ring state in completed 10% steps; separate raster digits display the exact percentage.
The assets include icons from these free sources:

- [Font Awesome Free 6.7.2](https://fontawesome.com/license/free), Copyright 2024 Fonticons, Inc., CC BY 4.0: lock, unlock-keyhole, check, triangle-exclamation, hand, person-walking, and right-from-bracket icons in the backgrounds, doors, pressure-lock/check indicators, and enter/exit panels.
- [Tabler Icons](https://github.com/tabler/tabler-icons/blob/main/LICENSE), Copyright (c) 2020-2026 Paweł Kuna, MIT: hourglass-half in both wait panels.
- [Ionicons](https://github.com/ionic-team/ionicons/blob/main/LICENSE), Copyright (c) 2015-present Ionic (http://ionic.io/), MIT: man in both occupant chambers.
- [Slint logo](../../../../logo/README.md), Copyright © SixtyFPS GmbH <info@slint.dev>, CC BY ND 4.0: the logo in the demo title banner.

Icons were resized, recolored, and rasterized into the compositions.
The Ionicons man artwork was cropped to its bounds and its paths combined; the empty chamber uses a pale version.
The Slint logo was uniformly scaled and rasterized alongside the title text.
Original composition and other artwork are Copyright © SixtyFPS GmbH <info@slint.dev>, MIT.
The repository's `REUSE.toml` records these credits and licenses for each affected PNG, with license texts in `LICENSES/`.

Retain these attributions when redistributing the raster artwork.
The MIT source headers do not replace third-party artwork licenses.
