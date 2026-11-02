# Controls API Reference

Import styled controls and theme types from [`src/basic.slint`](src/basic.slint).
Import headless controls and surface interfaces from [`src/headless.slint`](src/headless.slint).
The controls implement the existing standard widget interfaces from [`std-widget-interfaces.slint`](../../../internal/compiler/widgets/common/std-widget-interfaces.slint).
These interfaces define the common compatibility API; headless bases can expose additional properties, callbacks, and functions for interaction and styling.
The additional members don't become requirements for other implementations of the standard interfaces.
Import those interfaces directly; neither entry point re-exports them.
Enable experimental Slint features to use interfaces and typed slots.

This reference covers additions to the standard widget interfaces, behavior differences, and styled defaults.
Each control links to its standard API and behavior documentation.
For examples and customization rules, see the [usage guide](README.md).

Defaults described as “state style” follow the active state of the selected `style`.

## Shared Styled Inputs

Button, CheckBox, Switch, ComboBox, and LineEdit accept these `in` properties:

| Property | Type | Default |
| --- | --- | --- |
| `font-family` | `string` | Application default font |
| `font-size` | `length` | `14px` |
| `font-weight` | `int` | `400` |
| `font-italic` | `bool` | `false` |
| `horizontal-padding` | `length` | See each control |
| `vertical-padding` | `length` | See each control |
| `left-padding`, `right-padding` | `length` | `horizontal-padding` |
| `top-padding`, `bottom-padding` | `length` | `vertical-padding` |
| `spacing` | `length` | See each control |

All six styled controls accept `focus-width: length = 2px` and `focus-offset: length = 2px`.
Set `focus-width` to zero to hide the focus outline.
Focus color comes from `style.focus-color`.

Direct color inputs apply while enabled; disabled colors come from the style.
Geometry overrides apply in every state.
State priority is disabled, pressed, then hover; focus is independent.
Focus outlines don't change layout dimensions.

## Button

Sources: [styled](src/basic/button.slint), [headless](src/headless/button_base.slint), [standard interface](../../../internal/compiler/widgets/interfaces/button.slint), [standard behavior](../../../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/button.mdx).

### Additions: Button and ButtonBase

| Direction | Property | Type | Default or Meaning |
| --- | --- | --- | --- |
| `in` | `focus-on-click` | `bool` | `false`; takes focus on a left-button press when enabled |
| `in` | `focus-on-tab-navigation` | `bool` | `true`; participates in Tab navigation |
| `out` | `hovered` | `bool` | Pointer is over the enabled control |
| `out` | `focus-visible` | `bool` | Focus came from Tab navigation; pointer presses clear it |

Public function: `activate()` toggles `checked` when checkable, then emits `clicked()` when enabled.
ButtonBase slots: `background`, `content`.
Button supplies default visuals for both slots; callers can replace them with `<<`.
ButtonBase defaults both stretch factors to zero; set either factor to one to fill available layout space.

### Styled Inputs

Button includes the shared styled inputs above.

| Input | Type | Default |
| --- | --- | --- |
| `style` | `ButtonStyle` | `ButtonStyles.primary` when primary or checked; otherwise `ButtonStyles.secondary` |
| `background-color`, `foreground-color` | `brush` | State style |
| `icon-color` | `brush` | `foreground-color` |
| `icon-size` | `length` | `20px` |
| `spacing` | `length` | `6px` |
| `horizontal-padding`, `vertical-padding` | `length` | `12px`, `6px` |
| `border-width`, `border-radius` | `length` | `1px`, `4px` |

Minimum size follows the text, icon, spacing, and padding.
Missing icons and empty text don't create spacing gaps.
Icons retain their original colors unless `colorize-icon` is true, including while disabled.
When tinting is enabled, `icon-color` applies while enabled; disabled icons use the style's disabled foreground color.
Set `icon-color: transparent` to retain original image colors while enabled.

## CheckBox

Sources: [styled](src/basic/check_box.slint), [headless](src/headless/check_box_base.slint), [standard interface](../../../internal/compiler/widgets/interfaces/checkbox.slint), [standard behavior](../../../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/checkbox.mdx).

### Additions: CheckBox and CheckBoxBase

| Direction | Property | Type | Default or Meaning |
| --- | --- | --- | --- |
| `in` | `focus-on-click`, `focus-on-tab-navigation` | `bool` | `false`, `true`, as in ButtonBase |
| `out` | `hovered`, `pressed` | `bool` | Current interaction state |

Public function: `activate()` changes `checked` and emits `toggled()` when enabled.
CheckBoxBase also accepts the seven padding and spacing inputs from the shared table, defaulting to zero.
Use the `content` slot to arrange custom visuals and apply padding and spacing.
CheckBoxBase defaults both stretch factors to zero.

### Styled Inputs

CheckBox includes the shared styled inputs above.

| Input | Type | Default |
| --- | --- | --- |
| `style` | `CheckBoxStyle` | `CheckBoxStyles.standard` |
| `checked-color` | `brush` | State style's `indicator-color` |
| `foreground-color` | `brush` | State style |
| `spacing` | `length` | `6px`, or `0px` when text is empty |
| `horizontal-padding`, `vertical-padding` | `length` | `6px`, `6px` |
| `indicator-size` | `length` | `20px` |
| `indicator-border-width`, `indicator-radius` | `length` | `1px`, `4px` |
| `checkmark-padding`, `checkmark-width` | `length` | `3px`, `2px` |

Minimum size follows the indicator, label, and padding.
The styled control places its indicator beside the label, centered vertically.
`checked-color` overrides the indicator fill only while enabled and checked.

## Switch

Sources: [styled](src/basic/switch.slint), [headless](src/headless/switch_base.slint), [standard interface](../../../internal/compiler/widgets/interfaces/switch.slint), [standard behavior](../../../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/switch.mdx).

### Additions: Switch and SwitchBase

Switch and SwitchBase expose the same additions, padding inputs, and slots as CheckBox and CheckBoxBase.
They implement `SwitchInterface` and expose an additional `out property <bool> focus-visible` for keyboard focus indication.
`focus-visible` isn't part of `SwitchInterface`.

### Styled Inputs

Switch includes the shared styled inputs above.

| Input | Type | Default |
| --- | --- | --- |
| `style` | `SwitchStyle` | `SwitchStyles.standard` |
| `checked-color` | `brush` | State style's `track-color` |
| `foreground-color` | `brush` | State style |
| `spacing` | `length` | `6px`, or `0px` when text is empty |
| `horizontal-padding`, `vertical-padding` | `length` | `6px`, `5px` |
| `track-width`, `track-height` | `length` | `40px`, `22px` |
| `track-border-width` | `length` | `1px` |
| `track-radius`, `thumb-radius` | `length` | `-1px` for automatic rounding |
| `thumb-padding` | `length` | `3px` |
| `thumb-size` | `length` | `max(0px, track-height - 2 * thumb-padding)` |

Minimum size follows the track, label, and padding.
The styled control places its track beside the label, centered vertically.
The thumb's rendered size is bounded by the track's inner dimensions.
`checked-color` overrides the track fill only while enabled and checked.

## Slider

Sources: [styled](src/basic/slider.slint), [headless](src/headless/slider_base.slint), [standard interface](../../../internal/compiler/widgets/interfaces/slider.slint), [standard behavior](../../../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/slider.mdx).

### Additions: Slider and SliderBase

| Direction | Property | Type | Default or Meaning |
| --- | --- | --- | --- |
| `in` | `page-step` | `float` | Ten keyboard steps; keyboard step is `step`, or 1% of the range when `step` is zero or negative |
| `in` | `inverted` | `bool` | `false` |
| `out` | `hovered`, `pressed` | `bool` | Current interaction state |
| `out` | `position` | `float` | Bounded fraction from the lower to upper bound, from `0` to `1` |
| `out` | `visual-position` | `float` | Position adjusted for orientation and inversion |

| Public Function | Effect |
| --- | --- |
| `set-value(requested: float)` | Clamps and snaps while enabled; emits `changed` only if the stored value changes |
| `set-position(position: float)` | Converts a bounded fraction to a value, then calls `set-value` |
| `emit-released()` | Emits `released(value)` while enabled; this function isn't part of `SliderInterface` |

Assigning `value` directly doesn't clamp, snap, or emit callbacks.
The displayed and accessible values remain bounded.
`set-value` preserves exact endpoints and doesn't emit `released`.
Snapping is relative to the lower bound when `step` is positive.
Zero or negative `step` disables snapping and uses 1% of the range for keyboard steps.
SliderBase accepts a typed `surface` slot implementing `SliderSurface`.

### Styled Inputs

Slider accepts the shared focus inputs; it has no font or padding inputs.

| Input | Type | Default |
| --- | --- | --- |
| `style` | `SliderStyle` | `SliderStyles.standard` |
| `track-color`, `fill-color`, `handle-color` | `brush` | State style |
| `preferred-length`, `min-thickness` | `length` | `200px`, `32px`, following orientation |
| `handle-width`, `handle-height` | `length` | `20px`, `20px` |
| `handle-border-width` | `length` | `1px` |
| `handle-radius`, `track-radius` | `length` | `-1px` for automatic rounding |
| `track-thickness` | `length` | `4px` |

The minimum cross-axis size contains the handle and track, and is at least `min-thickness`.
The preferred size never falls below the minimum size.

### LinearSliderSurface and SliderSurface

Source: [LinearSliderSurface](src/headless/linear_slider_surface.slint).
SliderBase supplies the surface interface inputs and handles its callbacks.

| Direction | Property | Type | Meaning |
| --- | --- | --- | --- |
| `in` | `enabled`, `focused`, `pressed`, `inverted` | `bool` | Behavior state supplied by SliderBase |
| `in` | `position` | `float` | Logical fraction from `0` to `1` |
| `in` | `orientation` | `Orientation` | Slider direction |
| `out` | `hovered` | `bool` | Pointer is over the enabled surface |

Callbacks: `started()`, `seek(position: float)`, `finished()`, `canceled()`.
LinearSliderSurface adds `handle-width`, `handle-height`, and `track-thickness` length inputs, all defaulting to zero.
It also exposes `visual-position: float` as an output and accepts `track` and `handle` slots.
Its geometry inputs determine both visual placement and pointer mapping.

## ComboBox

Sources: [styled](src/basic/combo_box.slint), [headless](src/headless/combo_box_base.slint), [standard interface](../../../internal/compiler/widgets/interfaces/combobox.slint), [standard behavior](../../../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/combobox.mdx).

### Additions and Differences: ComboBox and ComboBoxBase

| Direction | Property | Type | Default or Meaning |
| --- | --- | --- | --- |
| `out` | `count` | `int` | Model length |
| `out` | `highlighted-index` | `int` | Highlighted popup row; initially `-1` |
| `out` | `expanded` | `bool` | Popup is open |
| `out` | `hovered`, `pressed` | `bool` | Current interaction state |
| `in` | `maximum-visible-items` | `int` | `8` |
| `in` | `popup-width` | `length` | Field width |
| `in` | `popup-height` | `length` | Visible row count times `item-height`, plus twice `popup-padding` |
| `in` | `item-height`, `popup-padding` | `length` | Zero in ComboBoxBase; styled defaults below |

ComboBoxBase also accepts the seven padding and spacing inputs from the shared table, defaulting to zero.
An empty model uses `current-index: -1`.
While the popup is open, keyboard navigation changes the highlight; Enter or Space commits it, and Escape closes without committing.
Clearing an open model closes the popup and restores field focus.

Additional callback: `highlighted(index: int)`.

| Public Function | Effect |
| --- | --- |
| `open()` | Opens an enabled, nonempty, closed popup |
| `close()` | Closes the popup and restores field focus if enabled |
| `highlight(index: int)` | Highlights a valid row while enabled and open; emits `highlighted` when changed |
| `select(index: int)` | Selects a valid row while enabled, closes the popup, and emits `selected` |

ComboBoxBase slots: `background`, `content`, `indicator`, and typed `popup: ComboBoxPopupSurface`.

### Styled Inputs

ComboBox includes the shared styled inputs above.

| Input | Type | Default |
| --- | --- | --- |
| `placeholder-text` | `string` | Empty |
| `display-text` | `string` | Selected value, or placeholder when no valid selection exists |
| `style` | `ComboBoxStyle` | `ComboBoxStyles.standard` |
| `background-color`, `foreground-color` | `brush` | Field state style |
| `horizontal-padding`, `vertical-padding`, `spacing` | `length` | `12px`, `6px`, `6px` |
| `border-width`, `border-radius` | `length` | `1px`, `4px` |
| `indicator-width`, `indicator-stroke-width` | `length` | `16px`, `2px` |
| `popup-border-width`, `popup-border-radius` | `length` | `1px`, `4px` |
| `popup-offset`, `popup-padding` | `length` | `2px`, `4px` |
| `min-item-height` | `length` | `32px` |
| `item-horizontal-padding`, `item-vertical-padding` | `length` | `12px`, `6px` |
| `selected-item-font-weight` | `int` | `600` |
| `item-height` | `length` | Larger of `min-item-height` and normal/selected font height plus twice `item-vertical-padding` |

Minimum and preferred sizes follow the label, indicator, spacing, and padding.
Field fonts also apply to popup rows.
Field padding and direct field colors don't change popup row padding or colors.
An explicit `item-height` replaces automatic row sizing.
It determines popup height and keyboard scrolling.
Popup row colors come from `style.popup`; disabled colors take priority over highlighted and selected colors.
`popup-offset` sets the gap below the field.

### ComboBoxPopup and ComboBoxPopupBase

Sources: [styled popup](src/basic/combo_box.slint), [headless popup](src/headless/combo_box_popup_base.slint).
Both implement `ComboBoxPopupSurface`, which declares:

| Direction | Property | Type | Default in ComboBoxPopupBase |
| --- | --- | --- | --- |
| `in` | `enabled` | `bool` | `true` |
| `in` | `model` | `[string]` | Empty |
| `in` | `current-index`, `highlighted-index` | `int` | `0` |
| `in` | `anchor-height`, `popup-width`, `popup-height`, `popup-padding`, `item-height` | `length` | `0px` |
| `in` | `maximum-visible-items` | `int` | `8` |
| `out` | `opened`, `focused` | `bool` | Popup visibility and focus state |

Callbacks: `highlight(index: int)`, `selected(index: int)`, `key-event(event: KeyEvent) -> EventResult`.
Public functions: `open()` and `close()`.
`open()` requires an enabled popup with a nonempty model.
Both components additionally expose `count: int` as an output and `popup-offset: length` as an input.
ComboBoxPopupBase defaults `popup-offset` to zero and hosts supplied children inside its popup.

ComboBoxPopup adds these styled inputs:

| Input | Type | Default |
| --- | --- | --- |
| `style` | `ComboBoxPopupStyle` | `ComboBoxStyles.standard.popup` |
| `font-family`, `font-size`, `font-weight`, `font-italic` | As in shared font inputs | Same font defaults |
| `border-width`, `border-radius` | `length` | `1px`, `4px` |
| `popup-offset`, `popup-padding` | `length` | `2px`, `4px` |
| `min-item-height` | `length` | `32px` |
| `item-horizontal-padding`, `item-vertical-padding` | `length` | `12px`, `6px` |
| `selected-item-font-weight` | `int` | `600` |
| `item-height` | `length` | Automatic font-based height, as in ComboBox |

When used in ComboBoxBase's typed slot, the base supplies model, selection, geometry, and behavior bindings.
Standalone popup usage must supply popup dimensions and connect its selection and keyboard callbacks.

## LineEdit

Sources: [styled](src/basic/line_edit.slint), [headless](src/headless/line_edit_base.slint), [standard interface](../../../internal/compiler/widgets/interfaces/lineedit.slint), [standard behavior](../../../docs/astro/src/content/docs/reference/std-widgets/views/lineedit.mdx).

### Additions: LineEdit and LineEditBase

| Direction | Property | Type | Default or Meaning |
| --- | --- | --- | --- |
| `in-out` | `password-revealed` | `bool` | `false`; displays password text without changing the declared `input-type` |
| `out` | `hovered` | `bool` | Pointer is over the enabled field |

Password reveal applies only to `input-type: password` and resets on focus loss, disabling, or a change of input type.
Revealed text uses native text-input behavior, including copying and cutting; hidden passwords suppress those operations.

LineEditBase adds `font-weight: int`, `foreground-color: brush`, and both `selection-*-color: color` inputs, delegated to native TextInput.
It also accepts the seven padding and spacing inputs from the shared table, defaulting to zero.
Its slots are `background`, `placeholder`, `leading`, and `trailing`.

### Styled Inputs and Slots

LineEdit includes the shared styled inputs above.

| Input | Type | Default |
| --- | --- | --- |
| `style` | `LineEditStyle` | `LineEditStyles.standard` |
| `background-color`, `foreground-color` | `brush` | State style |
| `selection-background-color`, `selection-foreground-color` | `color` | Style |
| `horizontal-padding`, `vertical-padding`, `spacing` | `length` | `12px`, `6px`, `8px` |
| `border-width`, `border-radius` | `length` | `1px`, `4px` |

Default minimum size is at least `160px` wide and `34px` high, growing to accommodate the native input and slots.
LineEdit exposes `leading` and `trailing` slots; its background and placeholder visuals are supplied internally.
Derived components inherit these slots automatically.
Selection color overrides apply while enabled; disabled fields use style selection colors.
Read-only fields retain enabled style colors.

## Style Struct Fields

All types below are exported from `basic.slint`.
Omitted fields in ordinary struct literals use their type defaults, rather than the control's preset colors.
All color fields are `brush`, except LineEdit selection colors, which are `color`.

| Struct | Fields and Types |
| --- | --- |
| `ButtonStateStyle` | `background-color`, `foreground-color`, `border-color`: `brush` |
| `ButtonStyle` | `rest`, `hover`, `pressed`, `disabled`: `ButtonStateStyle`; `focus-color`: `brush` |
| `CheckBoxStateStyle` | `indicator-color`, `checkmark-color`, `foreground-color`, `indicator-border-color`: `brush` |
| `CheckBoxStateStyles` | `rest`, `hover`, `pressed`, `disabled`: `CheckBoxStateStyle` |
| `CheckBoxStyle` | `unchecked`, `checked`: `CheckBoxStateStyles`; `focus-color`: `brush` |
| `SwitchStateStyle` | `track-color`, `thumb-color`, `foreground-color`, `track-border-color`: `brush` |
| `SwitchStateStyles` | `rest`, `hover`, `pressed`, `disabled`: `SwitchStateStyle` |
| `SwitchStyle` | `unchecked`, `checked`: `SwitchStateStyles`; `focus-color`: `brush` |
| `SliderStateStyle` | `track-color`, `fill-color`, `handle-color`, `handle-border-color`: `brush` |
| `SliderStyle` | `rest`, `hover`, `pressed`, `disabled`: `SliderStateStyle`; `focus-color`: `brush` |
| `ComboBoxStateStyle` | `background-color`, `foreground-color`, `border-color`: `brush` |
| `ComboBoxItemStyle` | `background-color`, `foreground-color`: `brush` |
| `ComboBoxPopupStyle` | `background-color`, `border-color`: `brush`; `rest`, `highlighted`, `selected`, `disabled`: `ComboBoxItemStyle` |
| `ComboBoxStyle` | `rest`, `hover`, `pressed`, `disabled`: `ComboBoxStateStyle`; `popup`: `ComboBoxPopupStyle`; `focus-color`: `brush` |
| `LineEditStateStyle` | `background-color`, `foreground-color`, `border-color`, `placeholder-color`: `brush` |
| `LineEditStyle` | `rest`, `hover`, `disabled`: `LineEditStateStyle`; `selection-background-color`, `selection-foreground-color`: `color`; `focus-color`: `brush` |

Read-only preset globals expose `ButtonStyles.secondary`, `primary`, `danger`, and `invisible`.
`CheckBoxStyles`, `SwitchStyles`, `SliderStyles`, `ComboBoxStyles`, and `LineEditStyles` each expose `standard`.
Presets derive their colors from BasicTheme.

## Theme API

Source: [`theme.slint`](src/basic/theme.slint).
`BasicPalette` contains these `brush` fields:

| Group | Fields |
| --- | --- |
| Surfaces and text | `foreground`, `background`, `hover-background`, `pressed-background`, `border` |
| Accent | `accent`, `accent-hover`, `accent-pressed`, `accent-foreground` |
| Danger | `danger`, `danger-hover`, `danger-pressed`, `danger-foreground`, `danger-disabled-foreground` |
| Invisible controls | `invisible-hover-background`, `invisible-pressed-background` |
| Disabled controls | `disabled-background`, `disabled-foreground` |
| Focus | `focus` |

| Export | Public API |
| --- | --- |
| `BasicPalettes` | Read-only `light` and `dark`: `BasicPalette` |
| `BasicTheme` | `in palette: BasicPalette`, defaulting to `default-palette`; read-only `default-palette: BasicPalette = BasicPalettes.light` |
| `BasicTheme` | One read-only `brush` output for every BasicPalette field, bound to the selected palette |
| `BasicThemeProvider` | `in palette: BasicPalette`, bound to `BasicTheme.palette` |

For provider scope and light/dark setup, see [Application Theme](README.md#application-theme).
