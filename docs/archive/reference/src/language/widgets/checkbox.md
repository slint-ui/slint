<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
## `CheckBox`

Use a `CheckBox` to let the user select or deselect values, for example in a list with multiple options. Consider using a `Switch` element instead if the action resembles more something that's turned on or off.

### Properties

-   **`checked`**: (_inout_ _bool_): Whether the checkbox is checked or not (default: false).
-   **`enabled`**: (_in_ _bool_): Defaults to true. When false, the checkbox can't be pressed (default: true)
-   **`has-focus`**: (_out_ _bool_): Set to true when the checkbox has keyboard focus (default: false).
-   **`text`** (_in_ _string_): The text written next to the checkbox.

### Callbacks

-   **`toggled()`**: The checkbox value changed

### Example

```slint
import { CheckBox } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    CheckBox {
        width: parent.width;
        height: parent.height;
        text: "Hello World";
    }
}
```

