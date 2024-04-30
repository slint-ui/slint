<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
## `TextEdit`

Similar to [`LineEdit`](#lineedit), but can be used to enter several lines of text

_Note:_ The current implementation only implement very few basic shortcut. More
shortcut will be implemented in a future version: <https://github.com/slint-ui/slint/issues/474>

### Properties

-   **`font-size`** (_in_ _length_): the size of the font of the input text
-   **`text`** (_in-out_ _string_): The text being edited
-   **`has-focus`**: (_in_out_ _bool_): Set to true when the widget currently has the focus
-   **`enabled`**: (_in_ _bool_): Defaults to true. When false, nothing can be entered
-   **`read-only`** (_in_ _bool_): When set to true, text editing via keyboard and mouse is disabled but selecting text is still enabled as well as editing text programmatically (default value: `false`)
-   **`wrap`** (_in_ _enum [`TextWrap`](../builtins/enums.md#textwrap)_): The way the text wraps (default: word-wrap).
-   **`horizontal-alignment`** (_in_ _enum [`TextHorizontalAlignment`](../builtins/enums.md#texthorizontalalignment)_): The horizontal alignment of the text.

### Functions

-   **`focus()`** Call this function to focus the TextEdit and make it receive future keyboard events.
-   **`clear-focus()`** Call this function to remove keyboard focus from this `TextEdit` if it currently has the focus. See also [](../concepts/focus.md).
-   **`set-selection-offsets(int, int)`** Selects the text between two UTF-8 offsets.
-   **`select-all()`** Selects all text.
-   **`clear-selection()`** Clears the selection.
-   **`copy()`** Copies the selected text to the clipboard.
-   **`cut()`** Copies the selected text to the clipboard and removes it from the editable area.
-   **`paste()`** Pastes the text content of the clipboard at the cursor position.

### Callbacks

-   **`edited(string)`**: Emitted when the text has changed because the user modified it

### Example

```slint
import { TextEdit } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 200px;
    TextEdit {
        font-size: 14px;
        width: parent.width;
        height: parent.height;
        text: "Lorem ipsum dolor sit amet,\n consectetur adipisici elit";
    }
}
```
