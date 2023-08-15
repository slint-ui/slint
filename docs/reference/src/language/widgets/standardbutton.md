## `StandardButton`

The StandardButton looks like a button, but instead of customizing with `text` and `icon`,
it can used one of the pre-defined `kind` and the text and icon will depend on the style.

### Properties

-   **`enabled`**: (_in_ _bool_): Defaults to true. When false, the button can't be pressed
-   **`has-focus`**: (_out_ _bool_): Set to true when the button currently has the focus
-   **`kind`** (_in_ _enum [`StandardButtonKind`](../builtins/enums.md#standardbuttonkind)_): The kind of button, one of `ok` `cancel`, `apply`, `close`, `reset`, `help`, `yes`, `no,` `abort`, `retry` or `ignore`
-   **`pressed`**: (_out_ _bool_): Set to true when the button is pressed.

### Callbacks

-   **`clicked()`**

### Example

```slint
import { StandardButton, VerticalBox } from "std-widgets.slint";
export component Example inherits Window {
  VerticalBox {
    StandardButton { kind: ok; }
    StandardButton { kind: apply; }
    StandardButton { kind: cancel; }
  }
}
```
