<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Builtin Global Singletons

## `Palette`

`Palette` give access to different brushes that can be used to to create custom widgets that matches the colors of
the selected style e.g. fluent, cupertino, material or qt.

### Properties

-   **`background`** (_out_ _brush_): Defines the background of the window.
-   **`on-background`** (_out_ _brush_): Defines the text color used on `background`.
-   **`accent`** (_out_ _brush_): Defines the color of highlighted elements e.g. a primary button.
-   **`on-accent`** (_out_ _brush_): Defines the text color used on `accent`.
-   **`surface`** (_out_ _brush_): Defines the main widget background color e.g. for button.
-   **`on-surface`** (_out_ _brush_): Defines the text color used on `surface`.
-   **`border`** (_out_ _brush_): Defines the border color of widgets.
-   **`selection`** (_out_ _brush_): Defines the text selection color.

## `TextInputInterface`

The `TextInputInterface.text-input-focused` property can be used to find out if a `TextInput` element has the focus.
If you're implementing your own virtual keyboard, this property is an indicator whether the virtual keyboard should be shown or hidden.

### Properties

-   **`text-input-focused`** (_bool_): True if an `TextInput` element has the focus; false otherwise.

### Example

```slint
import { LineEdit } from "std-widgets.slint";

component VKB {
    Rectangle { background: yellow; }
}

export component Example inherits Window {
    width: 200px;
    height: 100px;
    VerticalLayout {
        LineEdit {}
        FocusScope {}
        if TextInputInterface.text-input-focused: VKB {}
    }
}
```
