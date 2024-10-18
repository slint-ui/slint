## `Text`

The `Text` element is responsible for rendering text. Besides the `text` property, that specifies which text to render,
it also allows configuring different visual aspects through the `font-family`, `font-size`, `font-weight`, `color`, and
`stroke` properties.

The `Text` element can break long text into multiple lines of text. A line feed character (`\n`) in the string of the `text`
property will trigger a manual line break. For automatic line breaking you need to set the `wrap` property to a value other than
`no-wrap`, and it's important to specify a `width` and `height` for the `Text` element, in order to know where to break. It's
recommended to place the `Text` element in a layout and let it set the `width` and `height` based on the available screen space
and the text itself.

### Properties

-   **`color`** (_in_ _brush_): The color of the text. (default value: depends on the style)
-   **`font-family`** (_in_ _string_): The name of the font family selected for rendering the text.
-   **`font-size`** (_in_ _length_): The font size of the text.
-   **`font-weight`** (_in_ _int_): The weight of the font. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
-   **`font-italic`** (_in_ _bool_): Whether or not the font face should be drawn italicized or not. (default value: false)
-   **`font-metrics`** (_out_ _struct [`FontMetrics`](../language/builtins/structs.md#fontmetrics)_): The design metrics of the font scaled to the font pixel size used by the element.
-   **`horizontal-alignment`** (_in_ _enum [`TextHorizontalAlignment`](../language/builtins/enums.md#texthorizontalalignment)_): The horizontal alignment of the text.
-   **`letter-spacing`** (_in_ _length_): The letter spacing allows changing the spacing between the glyphs. A positive value increases the spacing and a negative value decreases the distance. (default value: 0)
-   **`overflow`** (_in_ _enum [`TextOverflow`](../language/builtins/enums.md#textoverflow)_): What happens when the text overflows (default value: clip).
-   **`text`** (_in_ _[string](../syntax/types.md#strings)_): The text rendered.
-   **`vertical-alignment`** (_in_ _enum [`TextVerticalAlignment`](../language/builtins/enums.md#textverticalalignment)_): The vertical alignment of the text.
-   **`wrap`** (_in_ _enum [`TextWrap`](../language/builtins/enums.md#textwrap)_): The way the text wraps (default value: `no-wrap`).
-   **`stroke`** (_in_ _brush_): The brush used for the text outline (default value: `transparent`).
-   **`stroke-width`** (_in_ _length_): The width of the text outline. If the width is zero, then a hairline stroke (1 physical pixel) will be rendered.
-   **`stroke-style`** (_in_ _enum [`TextStrokeStyle`](../language/builtins/enums.md#textstrokestyle)_): The style/alignment of the text outline (default value: `outside`).
-   **`rotation-angle`** (_in_ _angle_), **`rotation-origin-x`** (_in_ _length_), **`rotation-origin-y`** (_in_ _length_):
    Rotates the text by the given angle around the specified origin point. The default origin point is the center of the element.
    When these properties are set, the `Text` can't have children.

### Example

This example shows the text "Hello World" in red, using the default font:

```slint
export component Example inherits Window {
    width: 270px;
    height: 100px;

    Text {
        x:0;y:0;
        text: "Hello World";
        color: red;
    }
}
```

This example breaks a longer paragraph of text into multiple lines, by setting a `wrap`
policy and assigning a limited `width` and enough `height` for the text to flow down:

```slint
export component Example inherits Window {
    width: 270px;
    height: 300px;

    Text {
        x:0;
        text: "This paragraph breaks into multiple lines of text";
        wrap: word-wrap;
        width: 150px;
        height: 100%;
    }
}
```
