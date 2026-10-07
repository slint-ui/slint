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

## Example

From the repository root, open the example in the visual editor:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "1"
$env:SLINT_BACKEND = "winit-skia"
cargo +1.95.0 run -p slint-editor -- tools/editor/controls/examples/buttons.slint
```
