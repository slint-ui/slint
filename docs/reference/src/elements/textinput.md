## `TextInput`

The `TextInput` is a lower-level item that shows text and allows entering text.

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

### Properties

-   **`color`** (_in_ _brush_): The color of the text (default value: depends on the style)
-   **`font-family`** (_in_ _string_): The name of the font family selected for rendering the text.
-   **`font-size`** (_in_ _length_): The font size of the text.
-   **`font-weight`** (_in_ _int_): The weight of the font. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
-   **`font-italic`** (_in_ _bool_): Whether or not the font face should be drawn italicized or not. (default value: false)
-   **`font-metrics`** (_out_ _struct [`FontMetrics`](../language/builtins/structs.md#fontmetrics)_): The design metrics of the font scaled to the font pixel size used by the element.
-   **`has-focus`** (_out_ _bool_): `TextInput` sets this to `true` when it's focused. Only then it receives [`KeyEvent`](../language/builtins/structs.md#keyevent)s.
-   **`horizontal-alignment`** (_in_ _enum [`TextHorizontalAlignment`](../language/builtins/enums.md#texthorizontalalignment)_): The horizontal alignment of the text.
-   **`input-type`** (_in_ _enum [`InputType`](../language/builtins/enums.md#inputtype)_): Use this to configure `TextInput` for editing special input, such as password fields. (default value: `text`)
-   **`letter-spacing`** (_in_ _length_): The letter spacing allows changing the spacing between the glyphs. A positive value increases the spacing and a negative value decreases the distance. (default value: 0)
-   **`read-only`** (_in_ _bool_): When set to `true`, text editing via keyboard and mouse is disabled but selecting text is still enabled as well as editing text programmatically. (default value: `false`)
-   **`selection-background-color`** (_in_ _color_): The background color of the selection.
-   **`selection-foreground-color`** (_in_ _color_): The foreground color of the selection.
-   **`single-line`** (_in_ _bool_): When set to `true`, the text is always rendered as a single line, regardless of new line separators in the text. (default value: `true`)
-   **`text-cursor-width`** (_in_ _length_): The width of the text cursor. (default value: provided at run-time by the selected widget style)
-   **`text`** (_in-out_ _string_): The text rendered and editable by the user.
-   **`vertical-alignment`** (_in_ _enum [`TextVerticalAlignment`](../language/builtins/enums.md#textverticalalignment)_): The vertical alignment of the text.
-   **`wrap`** (_in_ _enum [`TextWrap`](../language/builtins/enums.md#textwrap)_): The way the text input wraps. Only makes sense when `single-line` is false. (default value: no-wrap)

### Functions

-   **`focus()`** Call this function to focus the text input and make it receive future keyboard events.
-   **`clear-focus()`** Call this function to remove keyboard focus from this `TextInput` if it currently has the focus. See also [](../concepts/focus.md).
-   **`set-selection-offsets(int, int)`** Selects the text between two UTF-8 offsets.
-   **`select-all()`** Selects all text.
-   **`clear-selection()`** Clears the selection.
-   **`copy()`** Copies the selected text to the clipboard.
-   **`cut()`** Copies the selected text to the clipboard and removes it from the editable area.
-   **`paste()`** Pastes the text content of the clipboard at the cursor position.

### Callbacks

-   **`accepted()`**: Invoked when enter key is pressed.
-   **`cursor-position-changed(Point)`**: The cursor was moved to the new (x, y) position
    described by the [_`Point`_](../language/builtins/structs.md#point) argument.
-   **`edited()`**: Invoked when the text has changed because the user modified it.

### Example

```slint
export component Example inherits Window {
    width: 270px;
    height: 100px;

    TextInput {
        text: "Replace me with a name";
    }
}
```