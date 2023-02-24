# Builtin Structures

## `KeyboardModifiers`

This structure is generated as part of `KeyEvent`, to indicate which modifier keys
are pressed during the generation of a key event.

### Fields

-   **`control`** (_bool_): `true` if the control key is pressed. On macOS this corresponds to the command key.
-   **`alt`** (_bool_): `true` if alt key is pressed.
-   **`shift`** (_bool_): `true` if the shift key is pressed.
-   **`meta`** (_bool_): `true` if the windows key is pressed on Windows, or the control key on macOS.

## `KeyEvent`

This structure is generated and passed to the key press and release
callbacks of the `FocusScope` element.

### Fields

-   **`text`** (_string_): The string representation of the key
-   **`modifiers`** (_KeyboardModifiers_): The keyboard modifiers pressed during the event

## `Point`

This structure represents a point with x and y coordinate

### Fields

-   **`x`** (_length_)
-   **`y`** (_length_)

## `PointerEvent`

This structure is generated and passed to the `pointer-event` callback of the `TouchArea` element.

### Fields

-   **`kind`** (_enum PointerEventKind_): The kind of the event: one of the following
    -   `down`: The button was pressed.
    -   `up`: The button was released.
    -   `cancel`: Another element or window took hold of the grab. This applies to all pressed button and the `button` is not relevant.
-   **`button`** (_enum PointerEventButton_): The button that was pressed or released. `left`, `right`, `middle`, or `none`.

## `StandardListViewItem`

The `StandardListViewItem` is used to display items in the `StandardListView` and the `StandardTableView`.

### Fields

-   **`text`** (_string_): Describes the text of the item.

## `TableColumn`

`TableColumn` is used to define the column and the column header of a TableView.

### Fields

-   **`title`** (_string_): Describes the column header title.
-   **`min-width`** (_length_): Defines the minimum with of the column.
-   **`width`** (_length_): The current width of the column.
-   **`horizontal-stretch`** (_float_): Defines the horizontal stretch of the column.
-   **`sort-order`** (_`SortOrder`_): Describes the sort order of the column.
