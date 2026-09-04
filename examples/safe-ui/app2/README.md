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
The assets include icons from these free sources:

- [Font Awesome Free](https://fontawesome.com/license/free): icons under CC BY 4.0.
- [Tabler Icons](https://github.com/tabler/tabler-icons/blob/main/LICENSE): MIT.
- [Ionicons](https://github.com/ionic-team/ionicons/blob/main/LICENSE): MIT.

Retain these attributions when redistributing the raster artwork.
The MIT source headers do not replace third-party artwork licenses.
