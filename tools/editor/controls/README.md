# Basic and Headless Controls

Import styled controls, styles, and theme types from `src/basic.slint`.
Import behavior components and slot surfaces from `src/headless.slint` when supplying your own visuals.
These controls require experimental Slint features for interfaces and typed slots.
Example import paths below are relative to `tools/editor/controls/examples/`.
See the [API reference](API.md) for each control's inputs, outputs, callbacks, functions, slots, and style fields.

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

Set `enabled: false` to disable interaction.
Supply `accessible-label` for controls without a text label, including icon-only buttons, sliders, and combo boxes.

Button, CheckBox, and Switch default both layout stretch factors to zero.
Set `horizontal-stretch` or `vertical-stretch` to one to fill extra space.
They participate in Tab navigation and preserve the current input focus when clicked.
Set `focus-on-click: true` for click focus, or `focus-on-tab-navigation: false` for actions outside the Tab order.

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

Use direct properties, such as `foreground-color` or `checked-color`, for simple color changes.
Direct colors override the selected style while enabled, including hover and pressed states.
Disabled controls use their style's disabled colors.

For different colors in each state, pass a complete style struct through `style`.
Struct literals don't merge with presets.
Omitted fields receive their declared defaults or their type's default value.
Reference preset fields explicitly to retain their values and reactive bindings.
Custom styles follow the application theme only where their bindings reference it.
See [Style Struct Fields](API.md#style-struct-fields) for the states and colors available on each control.

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

Use axis padding for symmetric spacing and edge padding for individual adjustments, as above.
Minimum dimensions allow content to grow; explicit dimensions determine the control's size.
See the [API reference](API.md) for each control's geometry inputs and defaults.

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

## Custom Visuals and Slots

Headless controls supply state, input handling, and accessibility while slot content supplies the visuals:

```slint
import { CheckBoxBase } from "../src/headless.slint";

choice := CheckBoxBase {
    text: "Show guides";
    spacing: 8px;
    content << HorizontalLayout {
        spacing: choice.spacing;
        cross-axis-alignment: center;
        Rectangle {
            width: 18px;
            height: 18px;
            background: choice.checked ? #2563eb : transparent;
            border-color: #2563eb;
            border-width: 1px;
        }
        Text { text: choice.text; }
    }
}
```

Basic controls fill their visual slots, except LineEdit's `leading` and `trailing`; use the corresponding headless base to replace other visuals.
Use the `content` slot to arrange custom visuals in ButtonBase, CheckBoxBase, and SwitchBase.
The [API reference](API.md) lists each base's slots and required geometry inputs.

For content around a styled text field, fill LineEdit's `leading` and `trailing` slots:

```slint
import { LineEdit } from "../src/basic.slint";

LineEdit {
    accessible-label: "File name";
    placeholder-text: "my-component";
    leading << Text { text: "File:"; }
    trailing << Text { text: ".slint"; }
}
```

Derived components must explicitly forward these slots to expose them; see `PreviewLineEdit` in `examples/preview_controls.slint`.

For password fields, set `input-type: password` and bind a reveal action to `password-revealed`.
Use an action that preserves input focus; revealing resets when the field loses focus.

Each control and its base implement the corresponding existing standard widget interface, such as `ButtonInterface` or `ComboBoxInterface`.
These interfaces define the shared compatibility contract, not the full API of each component.
Headless bases can expose additional properties, callbacks, and functions for interaction and styling.
For example, ComboBoxBase adds `highlighted-index` and the `highlighted` callback.
These additions aren't requirements for other standard widget implementations.
Import these directly from [`std-widget-interfaces.slint`](../../../internal/compiler/widgets/common/std-widget-interfaces.slint); the control entry points don't re-export them.
Use these interfaces for typed slots accepting styled controls or custom implementations.
Style definitions live in `src/basic/<control>_style.slint`.

## Gallery

Run the gallery with the checkout's viewer:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "1"
.\target\debug\slint-viewer.exe --auto-reload tools/editor/controls/examples/controls.slint
```

Each page provides size and disabled settings for previews, and the gallery supports light and dark palettes.
Preview sizes are defined by gallery components in `examples/preview_controls.slint`; headless examples keep their custom dimensions.
The Button page shows defaults, direct adjustments, reusable application defaults, and custom visuals through slots.
Its customization examples keep their own dimensions independently of the preview size selector.
Every control page demonstrates custom visuals through slots.
