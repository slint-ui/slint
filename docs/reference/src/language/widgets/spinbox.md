## `SpinBox`

### Properties

-   **`enabled`**: (_in_ _bool_): Defaults to true. You can't interact with the spinbox if enabled is false.
-   **`has-focus`**: (_out_ _bool_): Set to true when the spinbox currently has the focus
-   **`value`** (_in-out_ _int_): The value.
-   **`minimum`** (_in_ _int_): The minimum value (default: 0).
-   **`maximum`** (_in_ _int_): The maximum value (default: 100).

### Callbacks

- **`edited(`_`int`_`)`**: Emitted when the value has changed because the user modified it

### Example

```slint
import { SpinBox } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    SpinBox {
        width: parent.width;
        height: parent.height;
        value: 42;
    }
}
```
