# Basic Control Styles

## Choose a Customization Level

| Need | Use |
| --- | --- |
| A control with the default appearance | Import it from `src/basic.slint`. |
| A simple color change | Set a common color property, such as `CheckBox.checked-color`. |
| Different colors for interaction states | Pass the control's style struct through `style`. |
| Different dimensions, shapes, or typography | Pass size metrics through `size`, or override an individual font or layout property. |
| Shared application defaults | Reuse an application style through a small component, or update a control's global style preset. |
| Different content, layout, or visuals | Import the corresponding `*Base` from `src/headless.slint` and fill its slots. |

The basic controls already fill their visual slots.
Use the headless bases to replace those visuals; copying a basic control's implementation duplicates its styling logic.
Headless bases provide input behavior and accessibility while the application supplies visuals.
Set `accessible-label` when a control has no text label, especially for icon-only buttons, sliders, and combo boxes.
`LineEdit` also exposes optional `leading` and `trailing` slots around its native input.

The controls currently require experimental Slint features for interfaces and typed slots.
The examples below use paths relative to `tools/editor/controls/examples/`.

## Source Organization

For each control with style and size structs, `<control>_style.slint` defines the types.
`<control>_styles.slint` defines both the color and size presets, such as `ButtonStyles` and `ButtonSizes`.
Import controls, types, and presets through `src/basic.slint`.
The gallery owns the preview selector and selected index.

## Button

Import the button and presets from `src/basic.slint`:

```slint
import { Button, ButtonSizes, ButtonStyles } from "../src/basic.slint";

Button {
    text: "Save";
    style: ButtonStyles.primary;
    size: ButtonSizes.large;
}
```

`ButtonStyles` follows `BasicTheme`.
It exposes `secondary`, `primary`, `danger`, and `invisible` presets.
Primary uses green with white text.
Secondary uses light neutral surfaces, danger uses red, and invisible uses a transparent resting background.
Disabled primary and secondary buttons share neutral colors, and disabled danger buttons retain muted red text.

A `ButtonStyle` contains `rest`, `hover`, `pressed`, and `disabled` state styles.
Each state supplies background, foreground, and border colors.
Focus color belongs to the overall style.
Border width, border radius, focus width, and focus offset belong to `ButtonMetrics`.
Focus draws an independent outline outside the button, including while it is hovered or pressed.
The default outline is 2px wide with a 2px gap and does not change layout or the button's border.

## Simple Color Overrides

Common changes use direct properties:

```slint
import { CheckBox, CheckBoxSizes } from "../src/basic.slint";

CheckBox {
    text: "Notifications";
    checked: true;
    checked-color: #0f766e;
    size: CheckBoxSizes.small;
}
```

| Control | Common Color Properties |
| --- | --- |
| Button | `background-color`, `foreground` |
| CheckBox | `checked-color`, `foreground` |
| Switch | `checked-color`, `foreground` |
| Slider | `track-color`, `fill-color`, `handle-color` |
| ComboBox | `background-color`, `foreground` |

`checked-color` changes the checked indicator fill or switch track.
Border and checkmark colors still follow the style.
`foreground` changes the label or field text.
Direct colors apply while enabled, including hover and pressed states.
Disabled controls retain their style's disabled colors.
Use a complete style when each interaction state needs a different color.

## Custom Styles

Accept an application's style as a property and pass it to the button:

```slint
import { Button, ButtonStyle, ButtonStyles } from "../src/basic.slint";

export component StyledButtonExample inherits Window {
    in property <ButtonStyle> button-style: ButtonStyles.primary;

    Button {
        text: "Save";
        style: root.button-style;
    }
}
```

Struct literals do not merge with a preset.
Omitted fields receive their declared defaults or their type's default value.
Reference preset fields explicitly to reuse them.
Property bindings keep reused fields reactive when a theme changes.

## States and Overrides

Disabled takes priority over pressed and hover, in that order.
Rest is the default when none of these states applies.
Focus is independent and only appears on enabled buttons.

Set `enabled: false` to block activation.

Without an explicit `style`, `primary` or `checked` selects `ButtonStyles.primary`.
Other buttons use `ButtonStyles.secondary`.
An explicit style remains selected when `checked` changes.

Common color properties override the selected style while enabled.
Border colors, focus color, and per-state palettes belong to `style`.
Border widths, shapes, and focus dimensions belong to `size`.
The selected interaction state is private implementation detail.
General font and layout properties remain available as overrides.
The focus outline uses `style.focus-color`, `size.focus-width`, and `size.focus-offset`.
Set `size.focus-width` to zero to hide the outline.

## Sizes

The `size` property accepts a `ButtonMetrics` struct directly.
Use `ButtonSizes.small`, `ButtonSizes.medium`, or `ButtonSizes.large` for the predefined sizes.
The default, `ButtonSizes.medium`, preserves the original basic button's size defaults.
Pass a custom struct through `size` to supply your own metrics.

Metrics set minimum dimensions, horizontal and vertical padding, font size, icon size, and icon spacing.
They also set border width, border radius, focus width, and focus offset.
All three presets use a 1px border, 4px radius, 2px focus width, and 2px focus offset.
Transparent border colors hide borders on primary and invisible buttons without changing their geometry.
Style selection and size selection are independent.
Existing explicit dimensions, padding, and font properties remain available as overrides.
Minimum dimensions allow the control to grow to fit its content; they do not enforce an exact height or width.
Basic controls bind horizontal padding separately from `content-padding`.
Override `content-padding-left` and `content-padding-right` to change horizontal padding directly.

## Preview

`examples/controls.slint` shows the component gallery.
`ButtonExamples` owns the preview size, disabled setting, and activation count.
Its controls provide Small, Medium, and Large preview sizes.
Its Disabled checkbox disables the four preview buttons while keeping the settings available.
The button page shows primary, danger, secondary, and invisible styles.

For custom visuals using slots, open the CheckBox page's "Custom checkboxes" section.
In [check_boxes.slint](examples/check_boxes.slint), `CheckBoxBase` supplies the behavior while `indicator << Rectangle` and `content << Text` supply the visuals.
The Switch, Slider, ComboBox, and LineEdit pages also include slot-based examples.

Run the checkout's viewer with experimental features enabled:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "1"
.\target\debug\slint-viewer.exe --auto-reload tools/editor/controls/examples/controls.slint
```

## Switch, CheckBox, Slider, ComboBox, and LineEdit

These controls accept their own style and size structs through the same `style` and `size` properties.
Each style collection has one `standard` preset bound to `BasicTheme`.
Each size collection exposes `small`, `medium`, and `large` metrics, with medium as the default.

| Control | Style Type | Default Style | Size Type | Size Presets |
| --- | --- | --- | --- | --- |
| Switch | `SwitchStyle` | `SwitchStyles.standard` | `SwitchMetrics` | `SwitchSizes` |
| CheckBox | `CheckBoxStyle` | `CheckBoxStyles.standard` | `CheckBoxMetrics` | `CheckBoxSizes` |
| Slider | `SliderStyle` | `SliderStyles.standard` | `SliderMetrics` | `SliderSizes` |
| ComboBox | `ComboBoxStyle` | `ComboBoxStyles.standard` | `ComboBoxMetrics` | `ComboBoxSizes` |
| LineEdit | `LineEditStyle` | `LineEditStyles.standard` | `LineEditMetrics` | `LineEditSizes` |

```slint
import { Switch, SwitchStyle, SwitchStyles, SwitchSizes } from "../src/basic.slint";

export component StyledSwitchExample inherits Window {
    in property <SwitchStyle> switch-style: SwitchStyles.standard;

    Switch {
        text: "Notifications";
        style: root.switch-style;
        size: SwitchSizes.large;
    }
}
```

Switch and CheckBox styles contain `unchecked` and `checked` palettes.
Each palette contains `rest`, `hover`, `pressed`, and `disabled` state styles.
Changing `checked` selects the palette while preserving the current interaction state.
Switch states supply track, thumb, label, and border colors.
CheckBox states supply indicator, checkmark, label, and border colors.

Slider states supply track, filled track, handle, and handle border colors.
Slider sizes set handle dimensions, track thickness, minimum thickness, and preferred length for either orientation.

ComboBox styles contain field states and a `popup` style.
The popup defines its surface and `rest`, `highlighted`, `selected`, and `disabled` item styles.
Disabled items take priority over highlighted and selected items.
ComboBox sizes include field padding, font and indicator sizes, popup padding, and popup row height.
They also include field and popup border geometry, indicator stroke width, and normal and selected-item font weights.
`size.popup-offset` sets the gap below the field and defaults to 2px.
`font-weight` defaults to 400, and `selected-item-font-weight` defaults to 600.
Highlighted and disabled popup items use the normal font weight.
`ComboBoxPopup` also accepts a popup style and combo-box size metrics when used separately.

All five controls draw an independent focus outline using `style.focus-color`, `size.focus-width`, and `size.focus-offset`.
Use common color properties for simple changes and `style` for complete state palettes.
Geometry and typography belong to `size`.

| Control | Border and Shape Metrics |
| --- | --- |
| Button | `border-width`, `border-radius` |
| CheckBox | `indicator-border-width`, `indicator-radius` |
| Switch | `track-border-width`, `track-radius`, `thumb-radius` |
| Slider | `handle-border-width`, `handle-radius`, `track-radius` |
| ComboBox | `border-width`, `border-radius`, `popup-border-width`, `popup-border-radius` |
| LineEdit | `border-width`, `border-radius` |

Every size struct includes `focus-width` and `focus-offset`.
Switch sizes include `track-radius` and `thumb-radius`.
Slider sizes include `handle-radius` and `track-radius`.
These radii default to `-1px`, which follows the corresponding dimensions to produce rounded visuals.
Set a nonnegative radius for a fixed shape.
Basic controls expose indicator dimensions and popup dimensions for layout adjustments.
Explicit dimensions take priority over size presets.

The Button, Switch, CheckBox, Slider, ComboBox, and LineEdit pages own their preview size and disabled setting.
`PreviewSettings` shares the settings UI between these six pages.
Size selection applies to the basic styled controls; headless examples retain their custom dimensions.
The Disabled setting disables the examples while leaving the settings available.
The Switch, CheckBox, Slider, ComboBox, and LineEdit pages also accept a `preview-style` property for an application's style.

## LineEdit

Use style and size presets independently, including when adding content around the input:

```slint
import { LineEdit, LineEditStyles, LineEditSizes } from "../src/basic.slint";

LineEdit {
    accessible-label: "File name";
    placeholder-text: "my-component";
    style: LineEditStyles.standard;
    size: LineEditSizes.large;
    leading << Text { text: "File:"; }
    trailing << Text { text: ".slint"; }
}
```

`LineEditStyle` contains `rest`, `hover`, and `disabled` palettes with background, foreground, border, and placeholder colors.
Disabled takes priority over hover.
Read-only fields use the enabled palette and retain text selection and copying.
Selection colors and focus color belong to the overall style.
Selection colors use the `color` type to match the native input.
Focus draws an independent outline without changing the field border or text layout.

`LineEditMetrics` sets minimum dimensions, padding, spacing, font size and weight, border geometry, and focus dimensions.
The small, medium, and large presets use minimum heights of 26px, 34px, and 42px.
Medium preserves the original field's unfocused dimensions and typography.
Existing direct font, padding, border, and color overrides remain available.
Background, border, and placeholder color overrides apply while enabled; disabled fields use their style's palette.
`foreground` and selection color overrides configure the native input directly and remain in effect while disabled.
Use `style` for foreground colors that vary by state.

The basic control forwards `leading` and `trailing` slots while supplying the field background and placeholder.
Use `LineEditBase` when replacing those visuals.

## Share Checkbox Defaults

Use a small component to share a common color across any number of checkboxes:

```slint
import { CheckBox } from "../src/basic.slint";

global AppTheme {
    in-out property <brush> checkbox-color: #0f766e;
}

component AppCheckBox inherits CheckBox {
    checked-color: AppTheme.checkbox-color;
}

// Inside a layout:
AppCheckBox { text: "Notifications"; }
AppCheckBox { text: "Autosave"; }
```

Changing `AppTheme.checkbox-color` updates every `AppCheckBox`.
Other controls keep their colors.
The CheckBox gallery page demonstrates two instances sharing component defaults alongside a headless example with slots.

For full state palettes, define an application style and bind `style` in the same wrapper.
One appearance needs one shared style, regardless of the number of instances.
Changing `BasicTheme` affects all controls that use its tokens.
Changing `CheckBoxStyles.standard` affects basic checkboxes that use that preset.

## Migrate Individual Overrides

Keep the common color properties listed above.
Replace `CheckBox.indicator-color` and `Switch.track-color` with `checked-color` for checked fills, or use style fields for other states.
Move `Switch.track-width` and `track-height`, `CheckBox.indicator-size`, and Slider handle dimensions into size metrics.
Move detailed ComboBox popup colors into `style.popup`.
Move border widths, radii, focus width, and focus offset from styles into the corresponding size metrics.
Move ComboBox item font weights from `style.popup` into `size.font-weight` and `size.selected-item-font-weight`.
Use `LineEditStyle` and `LineEditMetrics` to share complete text-field appearances and dimensions.
Existing behavior properties, callbacks, accessibility, font overrides, and standard geometry remain available.

## Headless Controls

Import behavior components from `src/headless.slint` and styled controls from `src/basic.slint`.
Headless controls provide state, input handling, accessibility, and slots.
The basic controls supply colors, typography, dimensions, and visual defaults.

Size custom indicators inside their slots and add button padding through a layout in `ButtonBase.content`.
For example:

```slint
import { CheckBoxBase } from "../src/headless.slint";

choice := CheckBoxBase {
    text: "Show guides";
    spacing: 8px;
    indicator << Rectangle {
        width: 18px;
        height: 18px;
        vertical-stretch: 0;
        background: choice.checked ? #2563eb : transparent;
        border-color: #2563eb;
        border-width: 1px;
    }
    content << Text { text: choice.text; }
}
```

| Component | Slots |
| --- | --- |
| ButtonBase | `background`, `content` |
| CheckBoxBase, SwitchBase | `background`, `indicator`, `content` |
| ComboBoxBase | `background`, `content`, `indicator`, typed `popup` |
| LineEditBase | `background`, `placeholder`, `leading`, `trailing` |
| SliderBase | Typed `surface` |
| LinearSliderSurface | `track`, `handle` |

Derived components can add slots and forward them to these slots.

Headless padding and spacing default to zero.
Set combo-box `item-height`, `popup-padding`, and `ComboBoxPopupBase.popup-offset` explicitly when supplying custom popup visuals.
Set `LinearSliderSurface.handle-width`, `handle-height`, and `track-thickness` to match your visuals and preserve pointer mapping.
These popup and slider dimensions default to zero; basic controls supply their visual defaults.
