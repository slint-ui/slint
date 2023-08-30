<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
## `LineEdit`

A widget used to enter a single line of text. See [`TextEdit`](#textedit) for
a widget able to handle several lines of text.

### Properties

-   **`enabled`**: (_in_ _bool_): Defaults to true. When false, nothing can be entered selecting text is still enabled as well as editing text programmatically (default value: `false`)
-   **`font-size`** (_in_ _length_): the size of the font of the input text
-   **`has-focus`**: (_out_ _bool_): Set to true when the line edit currently has the focus
-   **`horizontal-alignment`** (_in_ _enum [`TextHorizontalAlignment`](../builtins/enums.md#texthorizontalalignment)_): The horizontal alignment of the text.
-   **`input-type`** (_in_ _enum [`InputType`](../builtins/enums.md#inputtype)_): The way to allow special input viewing properties such as password fields (default value: `text`).
-   **`placeholder-text`**: (_in_ _string_): A placeholder text being shown when there is no text in the edit field
-   **`read-only`** (_in_ _bool_): When set to true, text editing via keyboard and mouse is disabled but
-   **`text`** (_in-out_ _string_): The text being edited

### Functions

-   **`focus()`** Call this function to focus the LineEdit and make it receive future keyboard events.
-   **`select-all()`** Selects all text.
-   **`clear-selection()`** Clears the selection.
-   **`copy()`** Copies the selected text to the clipboard.
-   **`cut()`** Copies the selected text to the clipboard and removes it from the editable area.
-   **`paste()`** Pastes the text content of the clipboard at the cursor position.

### Callbacks

-   **`accepted(string)`**: Enter was pressed
-   **`edited(string)`**: Emitted when the text has changed because the user modified it

### Example

```slint
import { LineEdit } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    LineEdit {
        font-size: 14px;
        width: parent.width;
        height: parent.height;
        placeholder-text: "Enter text here";
    }
}
```
