## `GridBox`

A `GridBox` is a [`GridLayout`](../builtins/elements.md#gridlayout) where the spacing and padding values
depend on the style instead of defaulting to 0.

## `GroupBox`

A `GroupBox` is a container that groups its children together under a common title.

### Properties

-   **`enabled`**: (_in_ _bool_): Defaults to true. When false, the groupbox can't be interacted with
-   **`title`** (_in_ _string_): A text written as the title of the group box.

### Example

```slint
import { GroupBox , VerticalBox, CheckBox } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 100px;
    GroupBox {
        title: "Groceries";
        VerticalLayout {
            CheckBox { text: "Bread"; checked: true ;}
            CheckBox { text: "Fruits"; }
        }
    }
}
```
