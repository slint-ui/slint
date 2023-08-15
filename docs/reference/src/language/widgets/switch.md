## `Switch`

A `Switch` is a representation of a physical switch that allows users to turn things on or off. Consider using a `CheckBox` instead if you want the user to select or deselect values, for example in a list with multiple options.

### Properties

-   **`checked`**: (_inout_ _bool_): Whether the switch is checked or not (default: false).
-   **`enabled`**: (_in_ _bool_): Defaults to true. When false, the switch can't be pressed (default: true).
-   **`has-focus`**: (_out_ _bool_): Set to true when the switch has keyboard focue (default: false).
-   **`text`** (_in_ _string_): The text written next to the switch.

### Callbacks

-   **`toggled()`**: The switch value changed

### Example

```slint
import { Switch } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    Switch {
        width: parent.width;
        height: parent.height;
        text: "Hello World";
    }
}
```
