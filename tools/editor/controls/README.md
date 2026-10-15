# Basic Control Styles

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
Each state supplies a background, foreground, border color, and border width.
Focus color, focus width, focus offset, and border radius belong to the overall style.
Focus draws an independent outline outside the button, including while it is hovered or pressed.
The default outline is 2px wide with a 2px gap and does not change layout or the button's border.

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

Individual properties such as `background-color`, `foreground`, and `font-size` override their style defaults.
A fixed color override applies in every state.
Customize state styles when each state needs a different color.
The focus outline uses `style.focus-color`, `style.focus-width`, and `style.focus-offset`.
Set `style.focus-width` to zero to hide the outline.

## Sizes

The `size` property accepts a `ButtonSizeMetrics` struct directly.
Use `ButtonSizes.small`, `ButtonSizes.medium`, or `ButtonSizes.large` for the predefined sizes.
The default, `ButtonSizes.medium`, preserves the original basic button's size defaults.
Pass a custom struct through `size` to supply your own metrics.

Metrics set minimum dimensions, horizontal and vertical padding, font size, icon size, and icon spacing.
Style selection and size selection are independent.
Existing explicit dimensions, padding, and font properties remain available as overrides.

## Preview

`examples/controls.slint` shows the component gallery.
`ButtonExamples` owns the preview size, disabled setting, and activation count.
Its controls provide Small, Medium, and Large preview sizes.
Its Disabled checkbox disables the four preview buttons while keeping the settings available.
The button page shows primary, danger, secondary, and invisible styles.

`examples/styled-button.slint` shows one interactive button.
Run the checkout's viewer with experimental features enabled:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "1"
.\target\debug\slint-viewer.exe --auto-reload tools/editor/controls/examples/styled-button.slint
```

## Switch, CheckBox, Slider, and ComboBox

These controls accept their own style and size structs through the same `style` and `size` properties.
Each style collection has one `standard` preset bound to `BasicTheme`.
Each size collection exposes `small`, `medium`, and `large` metrics, with medium as the default.

| Control | Style Type | Default Style | Size Type | Size Presets |
| --- | --- | --- | --- | --- |
| Switch | `SwitchStyle` | `SwitchStyles.standard` | `SwitchSizeMetrics` | `SwitchSizes` |
| CheckBox | `CheckBoxStyle` | `CheckBoxStyles.standard` | `CheckBoxSizeMetrics` | `CheckBoxSizes` |
| Slider | `SliderStyle` | `SliderStyles.standard` | `SliderSizeMetrics` | `SliderSizes` |
| ComboBox | `ComboBoxStyle` | `ComboBoxStyles.standard` | `ComboBoxSizeMetrics` | `ComboBoxSizes` |

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
`ComboBoxPopup` also accepts a popup style and combo-box size metrics when used separately.

All four controls draw an independent focus outline using `focus-color`, `focus-width`, and `focus-offset`.
Existing individual color and dimension properties remain available as overrides.
Explicit dimensions take priority over size presets.

Each gallery page owns its preview size and disabled setting.
`PreviewSettings` shares the settings UI between these four pages.
Size selection applies to the basic styled controls; headless examples retain their custom dimensions.
The Disabled setting disables the examples while leaving the settings available.
Each page also accepts a `preview-style` property for an application's style.

