## `Window`

`Window` is the root of the tree of elements that are visible on the screen.

The `Window` geometry will be restricted by its layout constraints: Setting the `width` will result in a fixed width,
and the window manager will respect the `min-width` and `max-width` so the window can't be resized bigger
or smaller. The initial width can be controlled with the `preferred-width` property. The same applies to the `Window`s height.

### Properties

-   **`always-on-top`** (_in_ _bool_): Whether the window should be placed above all other windows on window managers supporting it.
-   **`background`** (_in_ _brush_): The background brush of the `Window`. (default value: depends on the style)
-   **`default-font-family`** (_in_ _string_): The font family to use as default in text elements inside this window, that don't have their `font-family` property set.
-   **`default-font-size`** (_in-out_ _length_): The font size to use as default in text elements inside this window, that don't have their `font-size` property set. The value of this property also forms the basis for relative font sizes.
-   **`default-font-weight`** (_in_ _int_): The font weight to use as default in text elements inside this window, that don't have their `font-weight` property set. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
-   **`icon`** (_in_ _image_): The window icon shown in the title bar or the task bar on window managers supporting it.
-   **`no-frame`** (_in_ _bool_): Whether the window should be borderless/frameless or not.
-   **`resize-border-width`** (_in_ _length_): Size of the resize border in borderless/frameless windows (winit only for now).
-   **`title`** (_in_ _string_): The window title that is shown in the title bar.
