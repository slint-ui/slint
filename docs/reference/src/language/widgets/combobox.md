<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
## `ComboBox`

A button that, when clicked, opens a popup to select a value.

### Properties

-   **`current-index`**: (_in-out_ _int_): The index of the selected value (-1 if no value is selected)
-   **`current-value`**: (_in-out_ _string_): The currently selected text
-   **`enabled`**: (_in_ _bool_): Defaults to true. When false, the combobox can't be interacted with
-   **`has-focus`**: (_out_ _bool_): Set to true when the combobox has keyboard focus.
-   **`model`** (_in_ _\[string\]_): The list of possible values

### Callbacks

-   **`selected(string)`**: A value was selected from the combo box. The argument is the currently selected value.

### Example

```slint
import { ComboBox } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 130px;
    ComboBox {
        y: 0px;
        width: self.preferred-width;
        height: self.preferred-height;
        model: ["first", "second", "third"];
        current-value: "first";
    }
}
```

