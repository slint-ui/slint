<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
## `StandardListView`

Like ListView, but with a default delegate, and a `model` property which is a model of type
[`StandardListViewItem`](../builtins/structs.md#standardlistviewitem).

### Properties

Same as [`ListView`](#listview), and in addition:

-   **`current-item`** (_in-out_ _int_): The index of the currently active item. -1 mean none is selected, which is the default
-   **`model`** (_in_ _[`StandardListViewItem`](../builtins/structs.md#standardlistviewitem)_): The model

### Functions

-   **`set-current-item(_index: int_)`**: Sets the current item and brings it into view

### Callbacks

-   **`current-item-changed(`_`int`_`)`**: Emitted when the current item has changed because the user modified it
-   **`item-pointer-event(`_`index: int`_`, `_`event: PointerEvent`_`, `_`pos: Point`_`)`**: Emitted on any mouse pointer event similar to `TouchArea`. Arguments are item index associated with the event, the `PointerEvent` itself and the mouse position within the listview.

### Example

```slint
import { StandardListView } from "std-widgets.slint";
export component Example inherits Window {
    width: 150px;
    height: 150px;
    StandardListView {
        width: 150px;
        height: 150px;
        model: [ { text: "Blue"}, { text: "Red" }, { text: "Green" },
            { text: "Yellow" }, { text: "Black"}, { text: "White"},
            { text: "Magenta" }, { text: "Cyan" },
        ];
    }
}
```
