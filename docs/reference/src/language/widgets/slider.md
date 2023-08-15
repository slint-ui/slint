## `Slider`

### Properties

-   **`enabled`**: (_in_ _bool_): Defaults to true. You can't interact with the slider if enabled is false.
-   **`has-focus`**: (_out_ _bool_): Set to true when the slider currently has the focus
-   **`value`** (_in-out_ _float_): The value.
-   **`minimum`** (_in_ _float_): The minimum value (default: 0)
-   **`maximum`** (_in_ _float_): The maximum value (default: 100)

### Callbacks

-   **`changed(float)`**: The value was changed

### Example

```slint
import { Slider } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    Slider {
        width: parent.width;
        height: parent.height;
        value: 42;
    }
}
```
