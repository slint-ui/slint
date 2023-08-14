# Font Handling

Elements such as `Text` and `TextInput` can render text and allow customizing the appearance of the text through
different properties. The properties prefixed with `font-`, such as `font-family`, `font-size` and `font-weight`
affect the choice of font used for rendering to the screen. If any of these properties isn't specified, the `default-font-`
values in the surrounding `Window` element apply, such as `default-font-family`.

The fonts chosen for rendering are automatically picked up from the system. It's also possible to include custom
fonts in your design. A custom font must be a TrueType font (`.ttf`), a TrueType font collection (`.ttc`) or an OpenType font (`.otf`).
You can select a custom font with the `import` statement: `import "./my_custom_font.ttf"` in a .slint file. This
instructions the Slint compiler to include the font and makes the font families globally available for use with
`font-family` properties.

For example:

```slint,ignore
import "./NotoSans-Regular.ttf";

export component Example inherits Window {
    default-font-family: "Noto Sans";

    Text {
        text: "Hello World";
    }
}
```
