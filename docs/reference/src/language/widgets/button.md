<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
## `Button`

A simple button. Common types of buttons can also be created with [`StandardButton`](#standardbutton).

### Properties

-   **`checkable`** (_in_ _bool_): Shows whether the button can be checked or not. This enables the `checked` property to possibly become true.
-   **`checked`** (_inout_ _bool_): Shows whether the button is checked or not. Needs `checkable` to be true to work.
-   **`enabled`**: (_in_ _bool_): Defaults to true. When false, the button cannot be pressed
-   **`has-focus`**: (_out_ _bool_): Set to true when the button has keyboard focus.
-   **`icon`** (_in_ _image_): The image to show in the button. Note that not all styles support drawing icons.
-   **`pressed`**: (_out_ _bool_): Set to true when the button is pressed.
-   **`text`** (_in_ _string_): The text written in the button.
-   **`primary`** (_in_ _bool_): If set to true the button is displayed with the primary accent color (default: false).
-  **`colorize-icon`** (_in_ _bool_): If set to true, the icon will be colorized to the same color as the Button's text color. (default: false)

### Callbacks

-   **`clicked()`**

### Example

```slint
import { Button, VerticalBox } from "std-widgets.slint";
export component Example inherits Window {
    VerticalBox {
        Button {
            text: "Click Me";
            clicked => { self.text = "Clicked"; }
        }
    }
}
```

