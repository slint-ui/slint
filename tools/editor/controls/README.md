# Basic and Headless Controls

Import styled controls, styles, and theme types from `src/basic.slint`.
Import behavior components and slot surfaces from `src/headless.slint` when supplying your own visuals.
These controls require experimental Slint features for interfaces and typed slots.
Example import paths below are relative to `tools/editor/controls/examples/`.

## Get Started

```slint
import { Button, ButtonStyles } from "../src/basic.slint";

export component App inherits Window {
    Button {
        text: "Save";
        style: ButtonStyles.primary;
        horizontal-padding: 16px;
    }
}
```

Every styled control supplies default colors and dimensions.
Set geometry directly on the control and use `style` for state colors.

| Control | Style Type | Default Style |
| --- | --- | --- |
| Button | `ButtonStyle` | `ButtonStyles.secondary` |
| CheckBox | `CheckBoxStyle` | `CheckBoxStyles.standard` |
| Switch | `SwitchStyle` | `SwitchStyles.standard` |
| Slider | `SliderStyle` | `SliderStyles.standard` |
| ComboBox | `ComboBoxStyle` | `ComboBoxStyles.standard` |
| LineEdit | `LineEditStyle` | `LineEditStyles.standard` |

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
Library palettes and style presets are read-only.
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
Button icons follow `foreground-color` unless `icon-color` is set.
Set `icon-color: transparent` to preserve the image's original colors while enabled; disabled icons use the style's disabled foreground.

For different colors in each state, pass a complete style struct through `style`.
Button, Slider, and ComboBox styles contain `rest`, `hover`, `pressed`, and `disabled` states.
CheckBox and Switch contain those states inside separate `unchecked` and `checked` palettes.
LineEdit uses `rest`, `hover`, and `disabled` states.
Disabled takes priority over pressed, then hover; focus draws an independent outline on enabled controls.

Struct literals don't merge with presets.
Omitted fields receive their declared defaults or their type's default value.
Reference preset fields explicitly to retain their values and reactive bindings.
Custom styles follow the application theme only where their bindings reference it.

## Dimensions and Padding

Set dimensions, padding, fonts, borders, and focus geometry directly:

```slint
Button {
    text: "Save";
    min-height: 28px;
    font-size: 13px;
    horizontal-padding: 12px;
    left-padding: 20px;
    border-radius: 0px;
}
```

Button, CheckBox, Switch, ComboBox, and LineEdit expose `horizontal-padding` and `vertical-padding`.
Each axis supplies its two edge defaults; `left-padding`, `right-padding`, `top-padding`, and `bottom-padding` override individual edges.
These controls also expose `font-family`, `font-size`, `font-weight`, and `font-italic`.
An empty font family uses the application's default font; normal weight defaults to 400.
CheckBox and Switch omit the label and default label spacing when `text` is empty.

Button, ComboBox, and LineEdit expose `border-width` and `border-radius` overrides that apply in every state.
CheckBox exposes `indicator-size`, `indicator-border-width`, `indicator-radius`, `checkmark-padding`, and `checkmark-width`.
Switch exposes track width, height, border width, and radius, plus thumb size, padding, and radius.
Its thumb size defaults to the track height minus twice the thumb padding.
Slider exposes handle width, height, border width, and radius, plus track thickness and radius.
Its `preferred-length` and `min-thickness` follow the selected orientation.
Minimum dimensions allow content to grow; explicit dimensions determine the control's size.
Focus uses `style.focus-color`, `focus-width`, and `focus-offset` without changing layout.
Set `focus-width` to zero to hide the outline.
Switch track/thumb and Slider track/handle radii use `-1px` for automatic rounding; nonnegative values give a fixed radius.

## Shared Control Defaults

Use an application component to share defaults across instances without changing other controls:

```slint
import { CheckBox } from "../src/basic.slint";

component AppCheckBox inherits CheckBox {
    checked-color: #0f766e;
    indicator-size: 18px;
    font-size: 13px;
    vertical-padding: 4px;
}

export component App inherits Window {
    VerticalLayout {
        AppCheckBox { text: "Notifications"; }
        AppCheckBox { text: "Autosave"; }
    }
}
```

Set `style` on the application component to share state colors as well.
Individual instances can override these defaults.

## Control-Specific Behavior

### ComboBox

Use `placeholder-text` when nothing is selected, or override `display-text` for a custom field label.
Direct field colors don't affect popup colors; configure those through `style.popup`.
Popup rows share the field's font settings; selected rows use `selected-item-font-weight`.
Disabled row colors take priority over highlighted and selected colors.
Default row height fits both fonts plus twice `item-vertical-padding`, with `min-item-height` as its minimum.
`item-horizontal-padding` controls row content padding; field padding doesn't affect popup rows.
An explicit `item-height` overrides automatic sizing and determines popup height and keyboard scrolling.
`popup-offset` sets the gap below the field; `popup-border-width` and `popup-border-radius` configure the popup border.
`indicator-width` and `indicator-stroke-width` configure the arrow.
Use `ComboBoxPopup` with a `ComboBoxPopupStyle` when supplying a custom field.
The standalone popup exposes `border-width`, `border-radius`, and the same row sizing properties.

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
Derived components must explicitly forward `leading` and `trailing` to expose them; see `PreviewLineEdit` in `examples/preview_controls.slint`.

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

Basic controls fill their visual slots, except LineEdit's `leading` and `trailing`; use the corresponding headless base to replace other visuals.
Size custom indicators inside their slots and add button padding through a layout in `ButtonBase.content`.
Headless padding and spacing default to zero.
Supply combo-box popup dimensions and linear-slider handle/track dimensions explicitly; their visual dimensions default to zero.
LineEditBase retains font, color, selection, and alignment inputs for its native `TextInput`.

Each control and its base share a behavior interface exported from both entry points, such as `ButtonInterface` or `ComboBoxInterface`.
Use these interfaces for typed slots accepting styled controls or custom implementations.
Interface definitions live in `src/interfaces/`; style definitions live in `src/basic/<control>_style.slint`.

## Gallery

Run the gallery with the checkout's viewer:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "1"
.\target\debug\slint-viewer.exe --auto-reload tools/editor/controls/examples/controls.slint
```

Each page provides size and disabled settings for previews, and the gallery supports light and dark palettes.
Preview sizes are defined by gallery components in `examples/preview_controls.slint`; headless examples keep their custom dimensions.
The CheckBox, Switch, Slider, ComboBox, and LineEdit pages demonstrate custom visuals through slots.
