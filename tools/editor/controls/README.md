<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Reusable Controls

Import `ButtonBase` from `src/headless.slint` to supply all visuals yourself.
It implements `ButtonInterface` from `std-widget-interfaces.slint`.
Styled buttons inherit this implementation and can expose additional properties and callbacks.
Import `Button` from `src/basic.slint` for default visuals and theme values.
The same entry points export `ComboBoxBase` and the styled `ComboBox`.
They also export `SliderBase` and the styled `Slider`.
Import `CheckBoxBase` from the headless entry point or `CheckBox` from the Basic entry point for boolean choices.
The same entry points export `SwitchBase` and the styled `Switch`.
Import `LineEditBase` for headless single-line editing or `LineEdit` for the Basic field style.

Headless implementations live in `src/headless/`.
The Basic style lives in `src/basic/`.
Examples live in `examples/`.

Button, CheckBox, and Switch bases share an internal `PressInteraction` component for pointer and keyboard handling.
Each base defines its own activation behavior, interface implementation, and accessibility role.
`PressInteraction` isn't exported from the headless entry point.

Enable `SLINT_ENABLE_EXPERIMENTAL_FEATURES=1` when loading these components.
They use experimental named and typed slots available on master.
They don't require slot-default compiler support.

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

## Customizing Controls

Use Basic controls when their styling properties cover the customization.
For example, change a button's `background-color`, `foreground`, border, font, or padding while keeping its built-in visuals.
The Basic style prioritizes disabled, pressed, and hovered states, followed by the normal appearance.
Primary and checked buttons use the accent colors within each interaction state.
Focus borders remain visible independently of those states.
Assigning `background-color` or `foreground` replaces its theme binding for every state.

Use the corresponding headless `*Base` component when you need different content, structure, or visuals.
For example, use `ButtonBase` for an icon-and-status layout or `LineEditBase` for leading and trailing adornments.
The base retains interaction, focus, callbacks, and accessibility.
Supply all required visuals through its existing slots and bind them to the base's state.

```slint
import { ButtonBase } from "../src/headless.slint";

action := ButtonBase {
    text: "Run";
    content-padding: 8px;
    background << Rectangle {
        background: action.pressed ? #0f766e : action.hovered ? #0d9488 : #14b8a6;
        border-radius: 4px;
        border-width: action.has-focus ? 2px : 0px;
        border-color: #134e4a;
    }
    content << Text {
        text: action.text;
        color: white;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
}
```

Basic controls fill their base slots with ordinary assignments.
Master's typed slots don't provide replaceable inherited default content, so callers can't replace those assignments through the base slot names.
Basic controls don't add `custom-*` slots or `use-default-*` switches.
This keeps one set of composition slots on the headless components.
Slot-default support would allow styled controls to offer replaceable defaults through those same slots; this implementation doesn't require it.

| Headless Component | Visual Slots |
| --- | --- |
| ButtonBase | `background`, `content` |
| CheckBoxBase, SwitchBase | `background`, `indicator`, `content` |
| LineEditBase | `background`, `placeholder`, `leading`, `trailing` |
| ComboBoxBase | `background`, `content`, `indicator`, typed `popup` |
| SliderBase | Typed `surface` |
| LinearSliderSurface | `track`, `handle` |

Untyped slots accept a single element or a layout containing several elements.
In a component derived from a base, an assignment fills that slot; further derived components can't replace it.
If a headless composition needs callers to supply a visual, leave the slot available or explicitly forward it through a slot on that composition.
The bases also accept ordinary children alongside their named content; `LinearSliderSurface` places ordinary children over its surface.

Content participates in layout with `content-padding` and its per-edge overrides.
Headless buttons measure the supplied content rather than their accessible label.
Basic buttons also supply a minimum size.
Override layout constraints when the design requires a different size.

`text` supplies the default accessible label even when custom content doesn't display it.
Override `accessible-label` when the action needs a different description.
Nested interactive controls handle their own activation and don't automatically activate the outer button.

## CheckBox Behavior

`CheckBoxBase` implements `CheckBoxInterface` from `std-widget-interfaces.slint`.
Pointer, keyboard, and accessibility activation toggle `checked`, then emit `toggled()` without an argument.
Read `checked` in the callback to obtain the new value.
Assigning `checked` directly doesn't emit `toggled()`.
`activate()` follows the same activation path and returns immediately when disabled.

The base handles pointer input with `TouchArea` and keyboard input with `FocusScope`.
Pointer clicks preserve keyboard focus.
Space and Enter toggle on release; Escape, focus loss, or disabling cancel a pending key press.

On the headless base, supply `background`, `indicator`, or `content` for custom visuals.
`hovered`, `pressed`, and `has-focus` expose interaction state.
`indicator-width`, `indicator-height`, `spacing`, and content padding arrange the indicator and label.
The Basic style adds indicator colors, borders, radius, size, and a focus ring.
`text` supplies the accessible label even when the content slot displays different visuals.

## Switch Behavior

`SwitchBase` implements `SwitchInterface` from `std-widget-interfaces.slint`.
It handles pointer input with `TouchArea` and keyboard input with `FocusScope`.
Activating an enabled switch toggles `checked`, then emits `toggled()` without an argument.
Assigning `checked` directly doesn't emit the callback.
`activate()` also supports programmatic activation through the same path.

Click the indicator or label to toggle.
Space and Enter toggle on release; key repeats don't toggle again.
Escape, focus loss, or disabling cancel a pending key press.
Pointer release outside the switch or losing the pointer grab cancels activation.
Pointer clicks preserve keyboard focus.

On the headless base, supply `background`, `indicator`, or `content` for custom visuals.
`hovered`, `pressed`, and `has-focus` expose interaction state.
The Basic style supplies a track, thumb, and focus ring, with configurable colors, dimensions, borders, and radii.
Its font properties belong to the styled component.
`text` supplies the accessible label even when custom content displays a different label.

## LineEdit Behavior

`LineEditBase` implements `LineEditInterface` from `std-widget-interfaces.slint`.
Its owned `TextInput` handles selection, keyboard input, input methods, clipboard operations, and undo/redo.
`text` exposes the current value; direct assignments don't emit `edited(text)`.
User edits emit `edited(text)`, and Enter emits `accepted(text)`.
The key callbacks can accept events before TextInput handles them.

Disabled fields reject user edits and clipboard mutations.
Read-only fields support focus, selection, and copying, but reject edits, cut, paste, undo, and redo.
`select-all()`, `clear-selection()`, and `set-selection-offsets(anchor, focus)` delegate to TextInput.
Clipboard and history methods also delegate, preserving TextInput's password protections.

Pointer clicks focus the field, unlike button-like controls.
The base exposes `has-focus`, `has-selection`, and `hovered`.
Long text scrolls horizontally to keep the caret visible.
The field exposes one accessible text input, including its value, placeholder, and read-only state.

On `LineEditBase`, supply `background`, `placeholder`, `leading`, and `trailing` without replacing the TextInput.
The placeholder appears only when the text and input-method preedit are empty.
Content padding and spacing arrange the editor and adornments.
The Basic style supplies foreground, selection, placeholder, border, and background colors.
The initial component doesn't provide a context menu.

## ComboBox Behavior

`ComboBoxBase` implements `ComboBoxInterface` from `std-widget-interfaces.slint`.
Set `model` to a list of strings.
`current-index` and `current-value` expose the selection; assigning either updates the other.
A value absent from the model clears the selection.
Use `current-index: -1` and `placeholder` to start without a selection.
Indices outside the model are clamped to `-1` or the last index; an empty model uses `-1`.
Programmatic changes don't emit `selected(current-value: string)`.
Pointer, keyboard, and accessibility selections emit it after committing the value.

Space, Enter, F4, and Alt+Down open the popup.
Arrows, Home, End, PageUp, and PageDown navigate its highlighted row.
Enter or Space commits that row; Escape closes without changing the selection.
When closed, navigation keys select directly.
Tab closes the popup and continues keyboard navigation.
Disabling the control, emptying its model, or moving focus outside the control and popup closes the popup.

On `ComboBoxBase`, supply `background`, `content`, and `indicator` to customize the field.
Supply the typed `popup` slot with a component implementing `ComboBoxPopupSurface`.
`ComboBoxPopupBase` provides the popup window, outside-click dismissal, and keyboard forwarding.
Its children provide the visuals and call `highlight(index)` and `selected(index)` for row interaction.
The Basic style supplies a scrollable `ComboBoxPopup` and exposes its colors on `ComboBox`.
Its popup slot is already filled; use `ComboBoxBase` to supply another popup surface.

## Slider Behavior

`SliderBase` implements `SliderInterface` from `std-widget-interfaces.slint` and handles range, keyboard, and accessibility interactions.
Its typed `surface` slot requires a component implementing `SliderSurface` for visuals and pointer mapping.
The styled `Slider` supplies a `LinearSliderSurface` with Basic theme visuals.
Its surface slot is already filled; use `SliderBase` to supply another surface.
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

The standalone gallery has Button, CheckBox, ComboBox, Slider, Switch, and LineEdit pages, each with Base theme and custom sections.
It includes default states, style overrides, replaced content, and custom headless visuals.
Run it from the repository root:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "1"
cargo +1.95.0 run --bin slint-viewer -- tools/editor/controls/examples/controls.slint
```
