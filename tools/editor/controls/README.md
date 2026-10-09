<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Reusable Controls

Import `ButtonBase` from `src/headless.slint` to supply all visuals yourself.
It implements `ButtonInterface` from `std-widget-interfaces.slint`.
Styled buttons inherit this implementation and can expose additional properties and callbacks.
Import `Button` from `src/basic.slint` for default visuals and theme values.
The same entry points export `ComboBoxBase` and the styled `ComboBox`.
They also export `SliderBase` and the styled `Slider`.

Headless implementations live in `src/headless/`.
The Basic style lives in `src/basic/`.
Examples live in `examples/`.

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
Pointer presses preserve the current keyboard focus.

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

## ComboBox Behavior

`ComboBoxBase` implements `ComboBoxInterface` from `std-widget-interfaces.slint`.
Set `model` to a list of strings.
`current-index` and `current-value` expose the selection; assigning either updates the other.
A value absent from the model clears the selection.
Use `current-index: -1` and `placeholder` to start without a selection.
Programmatic changes don't emit `selected(current-value: string)`.
Pointer, keyboard, and accessibility selections emit it after committing the value.

Space, Enter, F4, and Alt+Down open the popup.
Arrows, Home, End, PageUp, and PageDown navigate its highlighted row.
Enter or Space commits that row; Escape closes without changing the selection.
When closed, navigation keys select directly.
Tab closes the popup and continues keyboard navigation.
Disabling the control or emptying its model closes the popup.

Replace `background`, `content`, and `indicator` to customize the field.
Supply the typed `popup` slot with a component implementing `ComboBoxPopupSurface`.
`ComboBoxPopupBase` provides the popup window, outside-click dismissal, and keyboard forwarding.
Its children provide the visuals and call `highlight(index)` and `selected(index)` for row interaction.
The Basic style supplies a scrollable `ComboBoxPopup` and exposes its colors on `ComboBox`.

## Slider Behavior

`SliderBase` implements `SliderInterface` from `std-widget-interfaces.slint` and handles range, keyboard, and accessibility interactions.
Its typed `surface` slot requires a component implementing `SliderSurface` for visuals and pointer mapping.
The styled `Slider` supplies a `LinearSliderSurface` with Basic theme visuals.
Set `minimum`, `maximum`, `step`, and `value` to configure its range.
Assigning `value` directly updates the visuals without emitting either callback.
`set-value()` clamps to the range, snaps to a positive step, and emits `changed(value: float)` when the value changes.
`set-position()` uses a normalized position between zero and one.
Nonpositive steps allow continuous values; keyboard navigation uses one hundredth of the range.

Pointer release and adjustment-key release emit `released(value: float)`.
Accessibility adjustments emit both callbacks when the value changes, then finish with `released`.
Canceled pointer gestures and key presses canceled by focus loss or disabling don't emit `released`.
`emit-released()` lets custom interaction code finish an adjustment.

Use `orientation: vertical` for a vertical slider and `inverted: true` to reverse its visual direction.
Arrows adjust by one step; PageUp and PageDown use `page-step`; Home and End reach the range bounds.
Replace `track` and `handle` on `LinearSliderSurface` to supply custom visuals while keeping linear pointer mapping.
`position`, `visual-position`, `hovered`, `pressed`, and `has-focus` expose state for those visuals.
The Basic style exposes track, fill, and handle colors, dimensions, radii, and focus borders.

For other shapes, supply a different component through the typed `surface` slot.
The host supplies `enabled`, normalized `position`, `focused`, `pressed`, `orientation`, and `inverted`.
The surface exposes `hovered` and calls `seek(position)` to request a value change.
It calls `started()` on pointer press, then `finished()` on release or `canceled()` when canceled.
The host controls `pressed` so disabling the slider or pressing Escape cancels the surface's gesture.

The circular example in `examples/circular_surface.slint` maps pointer angles onto a 270-degree arc.
It shares a value with both custom linear sliders in the gallery.

```slint
import { SliderBase } from "../src/headless.slint";
import { CircularSurface } from "circular_surface.slint";

SliderBase {
    minimum: 0;
    maximum: 100;
    value: 35;
    surface << CircularSurface { }
}
```

## Example

The standalone gallery has Button, ComboBox, and Slider pages, each with Base theme and custom sections.
It includes default states, style overrides, replaced content, and custom headless visuals.
Run it from the repository root:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "1"
cargo +1.95.0 run --bin slint-viewer -- tools/editor/controls/examples/controls.slint
```
