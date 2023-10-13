<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

## `StandardTableView`

The `StandardTableView` represents a table of data with columns and rows. Cells
are organized in a model where each row is a model of
\[[`StandardListViewItem`](../builtins/structs.md#standardlistviewitem)\].

### Properties

Same as [`ListView`](#listview), and in addition:

-   **`current-sort-column`** (_out_ _int_): Indicates the sorted column. -1 mean no column is sorted.
-   **`columns`** (_in-out_ _\[[`TableColumn`](../builtins/structs.md#tablecolumn)\]_): Defines the model of the table columns.
-   **`rows`** (_\[\[[`StandardListViewItem`](../builtins/structs.md#standardlistviewitem)\]\]_): Defines the model of table rows.
-   **`current-row`** (_in-out_ _int_): The index of the currently active row. -1 mean none is selected, which is the default.

### Callbacks

-   **`sort-ascending(int)`**: Emitted if the model should be sorted by the given column in ascending order.
-   **`sort-descending(int)`**: Emitted if the model should be sorted by the given column in descending order.
-   **`row-pointer-event(int, PointerEvent, Point)`**: Emitted on any mouse pointer event similar to `TouchArea`. Arguments are row index associated with the event, the `PointerEvent` itself and the mouse position within the tableview.
-   **`current-row-changed(int)`**: Emitted when the current row has changed because the user modified it

### Functions

-   **`set-current-row(int)`**: Sets the current row by index and brings it into view.

### Example

```slint
import { StandardTableView } from "std-widgets.slint";
export component Example inherits Window {
    width: 230px;
    height: 200px;
    StandardTableView {
        width: 230px;
        height: 200px;
        columns: [
            { title: "Header 1" },
            { title: "Header 2" },
        ];
        rows: [
            [
                { text: "Item 1" }, { text: "Item 2" },
            ],
            [
                { text: "Item 1" }, { text: "Item 2" },
            ],
            [
                { text: "Item 1" }, { text: "Item 2" },
            ]
        ];
    }
}
```
