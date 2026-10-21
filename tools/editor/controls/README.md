# Basic and Headless Controls

Import styled controls, styles, size presets, and theme types from `src/basic.slint`.
Import behavior components and slot surfaces from `src/headless.slint` when supplying your own visuals.
These controls require experimental Slint features for interfaces and typed slots.
Example import paths below are relative to `tools/editor/controls/examples/`.

## Get Started

```slint
import { Button, ButtonStyles, ButtonSizes } from "../src/basic.slint";

export component App inherits Window {
    Button {
        text: "Save";
        style: ButtonStyles.primary;
        size: ButtonSizes.large;
    }
}
```

Every styled control works without specifying `style` or `size`.
All size collections provide `small`, `medium`, and `large` presets, with medium as the default.
Style and size selection are independent.

| Control | Style Type | Default Style | Metrics Type | Size Presets |
| --- | --- | --- | --- | --- |
| Button | `ButtonStyle` | `ButtonStyles.secondary` | `ButtonMetrics` | `ButtonSizes` |
| CheckBox | `CheckBoxStyle` | `CheckBoxStyles.standard` | `CheckBoxMetrics` | `CheckBoxSizes` |
| Switch | `SwitchStyle` | `SwitchStyles.standard` | `SwitchMetrics` | `SwitchSizes` |
| Slider | `SliderStyle` | `SliderStyles.standard` | `SliderMetrics` | `SliderSizes` |
| ComboBox | `ComboBoxStyle` | `ComboBoxStyles.standard` | `ComboBoxMetrics` | `ComboBoxSizes` |
| LineEdit | `LineEditStyle` | `LineEditStyles.standard` | `LineEditMetrics` | `LineEditSizes` |

Button also provides `primary`, `danger`, and `invisible` styles.
Set `enabled: false` to disable interaction.
Supply `accessible-label` for controls without a text label, including icon-only buttons, sliders, and combo boxes.

## Application Theme

Controls use `BasicPalettes.light` by default.
Select a palette through one `BasicThemeProvider` inside the root window:

```slint
import { BasicPalettes, BasicThemeProvider, Button } from "../src/basic.slint";

export component App inherits Window {
    in-out property <bool> dark-mode;

    BasicThemeProvider {
        palette: root.dark-mode ? BasicPalettes.dark : BasicPalettes.light;
    }
    Button { text: "Save"; }
}
```

The provider sets `BasicTheme.palette` for the whole window's component tree, including combo-box popups.
It doesn't scope the theme to its children; separate root windows have independent globals.
Bindings update controls when the selected palette changes.
Use the read-only colors on `BasicTheme` for application visuals that should follow that palette.
Host applications can export `BasicTheme` and set its `palette` input directly.

For custom colors, supply an application-owned `BasicPalette` instead of a library preset.
Supply every field, referencing preset fields for colors you want to keep; omitted fields become transparent.
Library palettes, style presets, and size presets are read-only.
The application selects light or dark mode; standard widgets from `std-widgets.slint` require their own palette configuration.

## Colors and State Styles

Use direct properties for simple color changes:

| Control | Color Inputs |
| --- | --- |
| Button | `background-color`, `foreground-color`, `icon-color` |
| CheckBox, Switch | `checked-color`, `foreground-color` |
| Slider | `track-color`, `fill-color`, `handle-color` |
| ComboBox, LineEdit | `background-color`, `foreground-color` |

Direct colors override the selected style while enabled, including hover and pressed states.
Disabled controls use their style's disabled colors.
`checked-color` changes the checked indicator or switch track fill; borders and checkmark colors still follow the style.
Button icons follow `foreground-color` unless `icon-color` is set; use `transparent` to preserve the image's original colors.

For different colors in each state, pass a complete style struct through `style`.
Button, Slider, and ComboBox styles contain `rest`, `hover`, `pressed`, and `disabled` states.
CheckBox and Switch contain those states inside separate `unchecked` and `checked` palettes.
LineEdit uses `rest`, `hover`, and `disabled` states.
Disabled takes priority over pressed, then hover; focus draws an independent outline on enabled controls.

Struct literals don't merge with presets.
Omitted fields receive their declared defaults or their type's default value.
Reference preset fields explicitly to retain their values and reactive bindings.
Custom styles follow the application theme only where their bindings reference it.

## Sizes and Individual Overrides

Pass metrics through `size` to share dimensions, padding, font sizes, border geometry, and focus dimensions.
Use direct properties for individual adjustments:

```slint
Button {
    text: "Save";
    size: ButtonSizes.small;
    horizontal-padding: 12px;
    left-padding: 20px;
    border-radius: 0px;
}
```

Button, CheckBox, Switch, ComboBox, and LineEdit expose `horizontal-padding` and `vertical-padding`, defaulting to their metrics.
Each axis supplies its two edge defaults; `left-padding`, `right-padding`, `top-padding`, and `bottom-padding` override individual edges.
These controls also expose `font-family`, `font-size`, `font-weight`, and `font-italic`.
An empty font family uses the application's default font; normal weight defaults to 400.
CheckBox and Switch omit the label and default label spacing when `text` is empty.

Button, ComboBox, and LineEdit expose `border-width` and `border-radius` overrides that apply in every state.
Detailed indicator, track, thumb, arrow, and popup geometry belongs to metrics.
Minimum dimensions allow content to grow; explicit dimensions determine the control's size.
Focus uses `style.focus-color`, `size.focus-width`, and `size.focus-offset` without changing layout.
Set `size.focus-width` to zero to hide the outline.
Switch track/thumb and Slider track/handle radii use `-1px` for automatic rounding; nonnegative values give a fixed radius.

## Shared Control Defaults

Use an application component to share defaults across instances without changing other controls:

```slint
import { CheckBox, CheckBoxSizes } from "../src/basic.slint";

component AppCheckBox inherits CheckBox {
    checked-color: #0f766e;
    size: CheckBoxSizes.small;
}

export component App inherits Window {
    VerticalLayout {
        AppCheckBox { text: "Notifications"; }
        AppCheckBox { text: "Autosave"; }
    }
}
```

Bind application-owned style and metrics structs in the same way when sharing a complete appearance.

## Control-Specific Behavior

### ComboBox

Use `placeholder-text` when nothing is selected, or override `display-text` for a custom field label.
Direct field colors don't affect popup colors; configure those through `style.popup`.
Popup rows share the field's font settings; selected rows use `size.selected-item-font-weight`.
Disabled row colors take priority over highlighted and selected colors.
Default row height fits the normal and selected fonts with vertical padding and is at least `size.item-height`.
An explicit `item-height` overrides automatic sizing and determines popup height and keyboard scrolling.
`size.popup-offset` sets the gap below the field.
Use `ComboBoxPopup` separately with a `ComboBoxPopupStyle` and `ComboBoxMetrics` when supplying a custom field.

### LineEdit

LineEdit exposes `leading` and `trailing` slots around its native input:

```slint
import { LineEdit } from "../src/basic.slint";

LineEdit {
    accessible-label: "File name";
    placeholder-text: "my-component";
    leading << Text { text: "File:"; }
    trailing << Text { text: ".slint"; }
}
```

Border and placeholder colors belong to the state style; selection colors belong to the overall style.
Direct selection color overrides apply while enabled; disabled fields use the style's selection colors.
Read-only fields retain enabled colors, text selection, and copying.

### Slider

Assigning or binding `value` stores it without clamping, snapping, or emitting `changed` or `released`.
The displayed position and accessible value remain bounded.
Call `set-value()` to clamp and snap relative to the lower bound when `step` is positive; exact endpoints remain reachable.
It does nothing while disabled and emits `changed(value)` only when the stored value changes.
It doesn't emit `released`; pointer, keyboard, and accessibility interactions emit that notification when they finish.

## Custom Visuals and Slots

Headless controls supply state, input handling, and accessibility while slot content supplies the visuals:

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

Basic controls already fill their visual slots; use the corresponding headless base to replace them.
Size custom indicators inside their slots and add button padding through a layout in `ButtonBase.content`.
Headless padding and spacing default to zero.
Supply combo-box popup dimensions and linear-slider handle/track dimensions explicitly; their visual dimensions default to zero.
LineEditBase retains font, color, selection, and alignment inputs for its native `TextInput`.

Each control and its base share a behavior interface exported from both entry points, such as `ButtonInterface` or `ComboBoxInterface`.
Use these interfaces for typed slots accepting styled controls or custom implementations.
Interface definitions live in `src/interfaces/`; style and metrics definitions live in `src/basic/<control>_style.slint`.

## Gallery

Run the gallery with the checkout's viewer:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "1"
.\target\debug\slint-viewer.exe --auto-reload tools/editor/controls/examples/controls.slint
```

Each page provides size and disabled settings for previews, and the gallery supports light and dark palettes.
Size selection applies to styled controls; headless examples keep their custom dimensions.
The CheckBox, Switch, Slider, ComboBox, and LineEdit pages demonstrate custom visuals through slots.
