<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Reusable Controls

Import `ButtonTemplate` from `src/templates.slint` to supply all visuals yourself.
Import `Button` from `src/basic.slint` for default visuals and theme values.
The editor's `EditorButton` adapts the Basic button to its theme.

Enable `SLINT_ENABLE_EXPERIMENTAL_FEATURES=1` when loading these components.
They use experimental named slots and default content.

## Button Behavior

`enabled` controls activation.
Set `checkable: true` to toggle `checked` on activation.
Assigning `checked` directly changes the state without emitting either callback.

`activate()` follows the same path as pointer, keyboard, and accessibility activation.
It returns immediately when the button is disabled.
For checkable buttons, it updates `checked` and emits `toggled(checked: bool)` before emitting `clicked()`.
For other buttons, it emits only `clicked()`.

`pressed`, `hovered`, and `has-focus` expose interaction state to custom visuals.
A pointer press shows `pressed` while inside the button.
Dragging outside clears it; returning inside restores it until release.
Releasing outside, losing the pointer grab, or disabling the button cancels pointer activation.
Re-enabling the button doesn't restore a canceled press.

Space and Enter activate on release while focused.
Key repeats don't cause repeated activation.
Escape, focus loss, or disabling the button cancels keyboard activation.
Tab navigation skips disabled buttons.
Set `focus-on-click: false` to activate without moving keyboard focus from another control.

## Replacing Visuals

The Basic style prioritizes disabled, pressed, and hovered states, followed by the normal appearance.
Primary and checked buttons use the accent colors within each interaction state.
Focus borders remain visible independently of those states.
Assign `background-color` or `foreground` to replace its theme binding for every state.

Replace `background` or `content` with a named slot assignment.
Derived components can replace their inherited defaults using a slot placeholder.
Both paths retain button behavior and accessibility.

The content participates in layout with `content-padding` and its per-edge overrides.
The Basic button supplies a minimum size; callers can override layout constraints in a derived component.
Replaced default content doesn't contribute to the button's preferred size.

`text` supplies the default accessible label even when custom content doesn't display it.
Override `accessible-label` when the action needs a different description.
Nested interactive controls handle their own activation and don't automatically activate the outer button.

## Slider Behavior

Import `SliderTemplate` from `src/templates.slint` for a linear slider without visuals.
Import `Slider` from `src/basic.slint` for a default track, filled range, and handle.
Both support horizontal and vertical orientations, and `inverted` reverses the direction.
Vertical sliders place the minimum at the bottom by default.

`minimum`, `maximum`, `step`, and `value` define the range.
Reversed bounds use the smaller value as the lower bound.
Positive steps snap relative to that lower bound; zero or negative steps allow continuous values.
Both endpoints remain reachable when the range isn't divisible by the step.
`position` is the normalized value, and `visual-position` accounts for orientation and inversion.
Values assigned outside the range display at the nearest endpoint.

Clicking the track sets the value immediately.
Dragging the handle preserves the pointer's grab offset and updates the value while dragging.
`pressed`, `hovered`, and `has-focus` expose interaction state.
Escape, pointer cancellation, or disabling the slider ends the drag and keeps the last value.

Arrow keys move by `step`, or one percent of the range for continuous sliders.
Inversion reverses arrow keys.
Page Up and Page Down use `page-step`, which defaults to ten keyboard steps.
Home and End select the lower and upper bounds.
Accessibility actions share the same clamping and snapping path.

`changed(value: float)` reports interaction changes, including `set-value()` and `set-position()` requests.
It fires only when the value changes.
Assigning `value` directly doesn't emit it.
Disabled sliders ignore interaction requests.

Replace `track` and `handle` to provide custom visuals while keeping linear interaction behavior.
The template places those visuals using `handle-width`, `handle-height`, and `track-thickness`.
The Basic style exposes track, fill, and handle brushes, radii, and handle border properties.
Caller brush bindings remain active across disabled, pressed, and hovered states.
Circular or other nonlinear pointer mappings need a separate interaction surface; changing the visuals alone doesn't change the linear mapping.

### Straight and Circular Surfaces

The [surface example](examples/slider-surfaces.slint) supplies straight and circular interaction surfaces to the same example host, `HeadlessSlider`.
Its typed `surface` slot requires `SliderSurface`: enabled state, normalized position, focus state, and a `seek` callback.
The host owns range limits, snapping, keyboard behavior, accessibility, and change callbacks.
This example host is separate from the reusable linear `SliderTemplate`.

`StraightSurface` maps horizontal pointer position to the value.
`CircularSurface` maps pointer angle along a 270-degree arc.
Requests in the bottom gap clamp to the nearest endpoint; pointer input at the center is ignored.
Both surfaces share the showcase value and enable toggle with the other sliders.

## Example

From the repository root, open the combined button, slider, and combo box showcase in the visual editor:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "1"
$env:SLINT_BACKEND = "winit-skia"
cargo +1.95.0 run -p slint-editor -- tools/editor/controls/examples/controls.slint
```

The `buttons.slint`, `sliders.slint`, and `combo-boxes.slint` files define sections of this single app.
