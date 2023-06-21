# Builtin Elements

## Common properties

### Geometry

These properties are valid on all visible items:

-   **`width`** and **`height`** (_in_ _length_): The size of the element. When set, this overrides the default size.
-   **`x`** and **`y`** (_in_ _length_): The position of the element relative to its parent.
-   **`z`** (_in_ _float_): Allows to specify a different order to stack the items with its siblings. (default value: 0)
-   **`absolute-position`** (_in_ _Point_): The position of the element within the contained window.

### Layout

These properties are valid on all visible items and can be used to specify constraints when used in layouts:

-   **`col`**, **`row`**, **`colspan`**, **`rowspan`** (_in_ _int_): See [`GridLayout`](#gridlayout).
-   **`horizontal-stretch`** and **`vertical-stretch`** (_in-out_ _float_): Specify how much relative space these elements are stretching in a layout. When 0, this means that the elements won't be stretched unless all elements are 0. Builtin widgets have a value of either 0 or 1.
-   **`max-width`** and **`max-height`** (_in_ _length_): The maximum size of an element
-   **`min-width`** and **`min-height`** (_in_ _length_): The minimum size of an element
-   **`preferred-width`** and **`preferred-height`** (_in_ _length_): The preferred size of an element

### Miscellaneous

-   **`cache-rendering-hint`** (_in_ _bool_): When set to `true`, this provides a hint to the renderer to cache the contents of the element and all the children into an intermediate cached layer. For complex sub-trees that rarely change this may speed up the rendering, at the expense of increased memory consumption. Not all rendering backends support this, so this is merely a hint. (default value: `false`)
-   **`dialog-button-role`** (_in_ _enum [`DialogButtonRole`](enums.md#dialogbuttonrole)_): Specify that this is a button in a `Dialog`.
-   **`opacity`** (_in_ _float_): A value between 0 and 1 (or a percentage) that is used to draw
    the element and its children with transparency.
    0 is fully transparent (invisible), and 1 is fully opaque.
    The opacity is applied to the tree of child elements as if they
    were first drawn into an intermediate layer, and then the whole layer is rendered with this opacity.
    (default value: 1)
-   **`visible`** (_in_ _bool_): When set to `false`, the element and all his children won't be drawn and not react to mouse input (default value: `true`)

The following example demonstrates the `opacity` property with children. An opacity is applied to the red rectangle. Since the green rectangle is a child of the red one, you can see the gradient underneath it, but you can't see the red rectangle through the green one.

```slint
export component Example inherits Window {
    width: 100px;
    height: 100px;
    background: @radial-gradient(circle, black, white, black, white);
    Rectangle {
        opacity: 0.5;
        background: red;
        border-color: #822;
        border-width: 5px;
        width: 50px; height: 50px;
        x: 10px; y: 10px;
        Rectangle {
            background: green;
            border-color: #050;
            border-width: 5px;
            width: 50px; height: 50px;
            x: 25px; y: 25px;
        }
    }
}
```

### Accessibility

Use the following `accessible-` properties to make your items interact well with software like screen readers, braille terminals and other software to make your application accessible.

-   **`accessible-role`** (_in_ _enum [`AccessibleRole`](enums.md#accessiblerole)_): The role of the element. This property is mandatory to be able to use any other accessible properties. It should be set to a constant value. (default value: `none` for most elements, but `text` for the Text element)
-   **`accessible-checkable`** (_in_ _bool_): Whether the element is can be checked or not.
-   **`accessible-checked`** (_in_ _bool_): Whether the element is checked or not. This maps to the "checked" state of checkboxes, radio buttons, and other widgets.
-   **`accessible-description`** (_in_ _string_): The description for the current element.
-   **`accessible-has-focus`** (_in_ _bool_): Set to true when the current element currently has the focus.
-   **`accessible-label`** (_in_ _string_): The label for an interactive element. (default value: empty for most elements, or the value of the `text` property for Text elements)
-   **`accessible-value-maximum`** (_in_ _float_): The maximum value of the item. This is used for example by spin boxes.
-   **`accessible-value-minimum`** (_in_ _float_): The minimum value of the item.
-   **`accessible-value-step`** (_in_ _float_) The smallest increment or decrement by which the current value can change. This corresponds to the step by which a handle on a slider can be dragged.
-   **`accessible-value`** (_in_ _string_): The current value of the item.

### Drop Shadows

To achieve the graphical effect of a visually elevated shape that shows a shadow effect underneath the frame of
an element, it's possible to set the following `drop-shadow` properties:

-   **`drop-shadow-blur`** (_in_ _length_): The radius of the shadow that also describes the level of blur applied to the shadow. Negative values are ignored and zero means no blur. (default value: 0)
-   **`drop-shadow-color`** (_in_ _color_): The base color of the shadow to use. Typically that color is the starting color of a gradient that fades into transparency.
-   **`drop-shadow-offset-x`** and **`drop-shadow-offset-y`** (_in_ _length_): The horizontal and vertical distance of the shadow from the element's frame. A negative value places the shadow left / above of the element.

The `drop-shadow` effect is supported for `Rectangle` elements.

## `Dialog`

Dialog is like a window, but it has buttons that are automatically laid out.

A Dialog should have one main element as child, that isn't a button.
The dialog can have any number of `StandardButton` widgets or other buttons
with the `dialog-button-role` property.
The buttons will be placed in an order that depends on the target platform at run-time.

The `kind` property of the `StandardButton`s and the `dialog-button-role` properties need to be set to a constant value, it can't be an arbitrary variable expression.
There can't be several `StandardButton`s of the same kind.

A callback `<kind>_clicked` is automatically added for each `StandardButton` which doesn't have an explicit
callback handler, so it can be handled from the native code: For example if there is a button of kind `cancel`,
a `cancel_clicked` callback will be added.

### Properties

-   **`icon`** (_in_ _image_): The window icon shown in the title bar or the task bar on window managers supporting it.
-   **`title`** (_in_ _string_): The window title that is shown in the title bar.

### Example

```slint
import { StandardButton, Button } from "std-widgets.slint";
export component Example inherits Dialog {
    Text {
      text: "This is a dialog box";
    }
    StandardButton { kind: ok; }
    StandardButton { kind: cancel; }
    Button {
      text: "More Info";
      dialog-button-role: action;
    }
}
```

## `Flickable`

The `Flickable` is a low-level element that is the base for scrollable
widgets, such as the [`ScrollView`](widgets.md#scrollview). When the `viewport-width` or the
`viewport-height` is greater than the parent's `width` or `height`
respectively, the element becomes scrollable. Note that the `Flickable`
doesn't create a scrollbar. When unset, the `viewport-width` and `viewport-height` are
calculated automatically based on the `Flickable`'s children. This isn't the
case when using a `for` loop to populate the elements. This is a bug tracked in
issue [#407](https://github.com/slint-ui/slint/issues/407).
The maximum and preferred size of the `Flickable` are based on the viewport.

When not part of a layout, its width or height defaults to 100% of the parent
element when not specified.

### Properties

-   **`interactive`** (_in_ _bool_): When true, the viewport can be scrolled by clicking on it and dragging it with the cursor. (default value: true)
-   **`viewport-height`**, **`viewport-width`** (_in_ _length_): The total size of the scrollable element.
-   **`viewport-x`**, **`viewport-y`** (_in_ _length_): The position of the scrollable element relative to the `Flickable`. This is usually a negative value.

### Example

```slint
export component Example inherits Window {
    width: 270px;
    height: 100px;

    Flickable {
        viewport-height: 300px;
        Text {
            x:0;
            y: 150px;
            text: "This is some text that you have to scroll to see";
        }
    }
}
```

## `FocusScope`

The `FocusScope` exposes callbacks to intercept key events. Note that `FocusScope`
will only invoke them when it `has-focus`.

The [`KeyEvent`](structs.md#keyevent) has a text property, which is a character of the key entered.
When a non-printable key is pressed, the character will be either a control character,
or it will be mapped to a private unicode character. The mapping of these non-printable, special characters is available in the [`Key`](namespaces.md#key) namespace

### Properties

-   **`has-focus`** (_out_ _bool_): Is `true` when the element has keyboard
    focus.
-   **`enabled`** (_in_ _bool_): When true, the `FocusScope` will make itself the focused element when clicked. Set this to false if you don't want the click-to-focus
    behavior. Similarly, a disabled `FocusScope` does not accept the focus via tab focus traversal. A parent `FocusScope` will still receive key events from
    child `FocusScope`s that were rejected, even if `enabled` is set to false. (default value: true)

### Functions

-   **`focus()`** Call this function to transfer keyboard focus to this `FocusScope`,
    to receive future [`KeyEvent`](structs.md#keyevent)s.

### Callbacks

-   **`key-pressed(`_[`KeyEvent`](structs.md#keyevent)_`) -> `[`EventResult`](structs.md#eventresult)**: Invoked when a key is pressed, the argument is a [`KeyEvent`](structs.md#keyevent) struct.
-   **`key-released(`_[`KeyEvent`](structs.md#keyevent)_`) -> `[`EventResult`](structs.md#eventresult)**: Invoked when a key is released, the argument is a [`KeyEvent`](structs.md#keyevent) struct.

### Example

```slint
export component Example inherits Window {
    width: 100px;
    height: 100px;
    forward-focus: my-key-handler;
    my-key-handler := FocusScope {
        key-pressed(event) => {
            debug(event.text);
            if (event.modifiers.control) {
                debug("control was pressed during this event");
            }
            if (event.text == Key.Escape) {
                debug("Esc key was pressed")
            }
            accept
        }
    }
}
```

## `GridLayout`

`GridLayout` places its children in a grid. `GridLayout` adds properties to each child: `col`, `row`, `colspan`, `rowspan`.
You can control the position of children with `col` and `row`.
If `col` or `row` aren't specified, they are automatically computed such that the item is next to the previous item, in the same row.
Alternatively, the item can be put in a `Row` element.

### Properties

-   **`spacing`** (_in_ _length_): The distance between the elements in the layout.
-   **`padding`** (_in_ _length_): The padding within the layout.
-   **`padding-left`**, **`padding-right`**, **`padding-top`** and **`padding-bottom`** (_in_ _length_):
    Set these properties to override the padding on specific sides.

### Examples

This example uses the `Row` element:

```slint
export component Foo inherits Window {
    width: 200px;
    height: 200px;
    GridLayout {
        spacing: 5px;
        Row {
            Rectangle { background: red; }
            Rectangle { background: blue; }
        }
        Row {
            Rectangle { background: yellow; }
            Rectangle { background: green; }
        }
    }
}
```

This example uses the `col` and `row` properties

```slint
export component Foo inherits Window {
    width: 200px;
    height: 150px;
    GridLayout {
        Rectangle { background: red; }
        Rectangle { background: blue; }
        Rectangle { background: yellow; row: 1; }
        Rectangle { background: green; }
        Rectangle { background: black; col: 2; row: 0; }
    }
}
```

## `Image`

An `Image` can be used to represent an image loaded from a file.

### Properties

-   **`colorize`** (_in_ _brush_): When set, the image is used as an alpha mask and is drawn in the given color (or with the gradient).
-   **`image-fit`** (_in_ _enum [`ImageFit`](enums.md#imagefit)_): Specifies how the source image shall be fit into the image element. (default value: `contain` when the `Image` element is part of a layout, `fill` otherwise)
-   **`image-rendering`** (_in_ _enum [`ImageRendering`](enums.md#imagerendering)_): Specifies how the source image will be scaled. (default value: `smooth`)
-   **`rotation-angle`** (_in_ _angle_), **`rotation-origin-x`** (_in_ _length_), **`rotation-origin-y`** (_in_ _length_):
    Rotates the image by the given angle around the specified origin point. The default origin point is the center of the element.
    When these properties are set, the `Image` can't have children.
-   **`source`** (_in_ _image_): The image to load. Use the `@image-url("...")` macro to specify the location of the image.
-   **`source-clip-x`**, **`source-clip-y`**, **`source-clip-width`**, **`source-clip-height`** (_in_ _int_): Properties in source
    image coordinates that define the region of the source image that is rendered. By default the entire source image is visible:
    | Property | Default Binding |
    |----------|---------------|
    | `source-clip-x` | `0` |
    | `source-clip-y` | `0` |
    | `source-clip-width` | `source.width - source-clip-x` |
    | `source-clip-height` | `source.height - source-clip-y` |
-   **`width`**, **`height`** (_in_ _length_): The width and height of the image as it appears on the screen.The default values are
    the sizes provided by the **`source`** image. If the `Image` is **not** in a layout and only **one** of the two sizes are
    specified, then the other defaults to the specified value scaled according to the aspect ratio of the **`source`** image.

### Example

```slint
export component Example inherits Window {
    width: 100px;
    height: 100px;
    VerticalLayout {
        Image {
            source: @image-url("https://slint.dev/logo/slint-logo-full-light.svg");
            // image-fit default is `contain` when in layout, preserving aspect ratio
        }
        Image {
            source: @image-url("https://slint.dev/logo/slint-logo-full-light.svg");
            colorize: red;
        }
    }
}
```

Scaled while preserving the aspect ratio:

```slint
export component Example inherits Window {
    width: 100px;
    height: 150px;
    VerticalLayout {
        Image {
            source: @image-url("https://slint.dev/logo/slint-logo-full-light.svg");
            width: 100px;
            // implicit default, preserving aspect ratio:
            // height: self.width * natural_height / natural_width;
        }
    }
}
```

## `Path`

The `Path` element allows rendering a generic shape, composed of different geometric commands. A path
shape can be filled and outlined.

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

A path can be defined in two different ways:

-   Using SVG path commands as a string
-   Using path command elements in `.slint` markup.

The coordinates used in the geometric commands are within the imaginary coordinate system of the path.
When rendering on the screen, the shape is drawn relative to the `x` and `y` properties. If the `width`
and `height` properties are non-zero, then the entire shape is fit into these bounds - by scaling
accordingly.

### Common Path Properties

-   **`fill`** (_in_ _brush_): The color for filling the shape of the path.
-   **`fill-rule`** (_in_ _enum [`FillRule`](enums.md#fillrule)_): The fill rule to use for the path. (default value: `nonzero`)
-   **`stroke`** (_in_ _brush_): The color for drawing the outline of the path.
-   **`stroke-width`** (_in_ _length_): The width of the outline.
-   **`width`** (_in_ _length_): If non-zero, the path will be scaled to fit into the specified width.
-   **`height`** (_in_ _length_): If non-zero, the path will be scaled to fit into the specified height.
-   **`viewbox-x`**/**`viewbox-y`**/**`viewbox-width`**/**`viewbox-height`** (_in_ _float_) These four
    properties allow defining the position and size of the viewport of the path in path coordinates.

    If the `viewbox-width` or `viewbox-height` is less or equal than zero, the viewbox properties are
    ignored and instead the bounding rectangle of all path elements is used to define the view port.

-   **`clip`** (_in_ _bool_): By default, when a path has a view box defined and the elements render
    outside of it, they are still rendered. When this property is set to `true`, then rendering will be
    clipped at the boundaries of the view box.
    This property must be a literal `true` or `false` (default value: `false`)

#### Path Using SVG commands

SVG is a popular file format for defining scalable graphics, which are often composed of paths. In SVG
paths are composed using [commands](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/d#path_commands),
which in turn are written in a string. In `.slint` the path commands are provided to the `commands`
property. The following example renders a shape consists of an arc and a rectangle, composed of `line-to`,
`move-to` and `arc` commands:

```slint
export component Example inherits Path {
    width: 100px;
    height: 100px;
    commands: "M 0 0 L 0 100 A 1 1 0 0 0 100 100 L 100 0 Z";
    stroke: red;
    stroke-width: 1px;
}
```

The commands are provided in a property:

-   **`commands`** (_in_ _string_): A string providing the commands according to the SVG path specification.

#### Path Using SVG Path Elements

The shape of the path can also be described using elements that resemble the SVG path commands but use the
`.slint` markup syntax. The earlier example using SVG commands can also be written like that:

```slint
export component Example inherits Path {
    width: 100px;
    height: 100px;
    stroke: blue;
    stroke-width: 1px;

    MoveTo {
        x: 0;
        y: 0;
    }
    LineTo {
        x: 0;
        y: 100;
    }
    ArcTo {
        radius-x: 1;
        radius-y: 1;
        x: 100;
        y: 100;
    }
    LineTo {
        x: 100;
        y: 0;
    }
    Close {
    }
}
```

Note how the coordinates of the path elements don't use units - they operate within the imaginary
coordinate system of the scalable path.

##### `MoveTo` Sub-element for `Path`

The `MoveTo` sub-element closes the current sub-path, if present, and moves the current point
to the location specified by the `x` and `y` properties. Subsequent elements such as `LineTo`
will use this new position as their starting point, therefore this starts a new sub-path.

###### Properties

-   **`x`** (_in_ _float_): The x position of the new current point.
-   **`y`** (_in_ _float_): The y position of the new current point.

##### `LineTo` Sub-element for `Path`

The `LineTo` sub-element describes a line from the path's current position to the
location specified by the `x` and `y` properties.

###### Properties

-   **`x`** (_in_ _float_): The target x position of the line.
-   **`y`** (_in_ _float_): The target y position of the line.

##### `ArcTo` Sub-element for `Path`

The `ArcTo` sub-element describes the portion of an ellipse. The arc is drawn from the path's
current position to the location specified by the `x` and `y` properties. The remaining properties
are modelled after the SVG specification and allow tuning visual features such as the direction
or angle.

###### Properties

-   **`large-arc`** (_in_ _bool_): Out of the two arcs of a closed ellipse, this flag selects that the larger arc is to be rendered. If the property is `false`, the shorter arc is rendered instead.
-   **`radius-x`** (_in_ _float_): The x-radius of the ellipse.
-   **`radius-y`** (_in_ _float_): The y-radius of the ellipse.
-   **`sweep`** (_in_ _bool_): If the property is `true`, the arc will be drawn as a clockwise turning arc; anti-clockwise otherwise.
-   **`x-rotation`** (_in_ _float_): The x-axis of the ellipse will be rotated by the value of this properties, specified in as angle in degrees from 0 to 360.
-   **`x`** (_in_ _float_): The target x position of the line.
-   **`y`** (_in_ _float_): The target y position of the line.

##### `CubicTo` Sub-element for `Path`

The `CubicTo` sub-element describes a smooth Bézier from the path's current position to the
location specified by the `x` and `y` properties, using two control points specified by their
respective properties.

###### Properties

-   **`control-1-x`** (_in_ _float_): The x coordinate of the curve's first control point.
-   **`control-1-y`** (_in_ _float_): The y coordinate of the curve's first control point.
-   **`control-2-x`** (_in_ _float_): The x coordinate of the curve's second control point.
-   **`control-2-y`** (_in_ _float_): The y coordinate of the curve's second control point.
-   **`x`** (_in_ _float_): The target x position of the curve.
-   **`y`** (_in_ _float_): The target y position of the curve.

##### `QuadraticTo` Sub-element for `Path`

The `QuadraticTo` sub-element describes a smooth Bézier from the path's current position to the
location specified by the `x` and `y` properties, using the control points specified by the
`control-x` and `control-y` properties.

###### Properties

-   **`control-x`** (_in_ _float_): The x coordinate of the curve's control point.
-   **`control-y`** (_in_ _float_): The y coordinate of the curve's control point.
-   **`x`** (_in_ _float_): The target x position of the curve.
-   **`y`** (_in_ _float_): The target y position of the curve.

##### `Close` Sub-element for `Path`

The `Close` element closes the current sub-path and draws a straight line from the current
position to the beginning of the path.

## `PopupWindow`

Use this element to show a popup window like a tooltip or a popup menu.

Note: It isn't allowed to access properties of elements within the popup from outside of the `PopupWindow`.

### Properties

-   **`close-on-click`** (_in_ _bool_): By default, a PopupWindow closes when the user clicks. Set this
    to false to prevent that behavior and close it manually using the `close()` function. (default value: true)

### Functions

-   **`show()`** Show the popup on the screen.
-   **`close()`** Closes the popup. Use this if you set the `close-on-click` property to false.

### Example

```slint
export component Example inherits Window {
    width: 100px;
    height: 100px;

    popup := PopupWindow {
        Rectangle { height:100%; width: 100%; background: yellow; }
        x: 20px; y: 20px; height: 50px; width: 50px;
    }

    TouchArea {
        height:100%; width: 100%;
        clicked => { popup.show(); }
    }
}
```

## `Rectangle`

By default, a `Rectangle` is just an empty item that shows nothing. By setting a color or configuring a border,
it's then possible to draw a rectangle on the screen.

When not part of a layout, its width and height default to 100% of the parent element.

### Properties

-   **`background`** (_in_ _brush_): The background brush of this `Rectangle`, typically a color. (default value: `transparent`)
-   **`border-color`** (_in_ _brush_): The color of the border. (default value: `transparent`)
-   **`border-radius`** (_in_ _length_): The size of the radius. (default value: 0)
-   **`border-width`** (_in_ _length_): The width of the border. (default value: 0)
-   **`clip`** (_in_ _bool_): By default, when an element is bigger or outside another element, it's still shown. When this property is set to `true`, the children of this `Rectangle` are clipped to the border of the rectangle. (default value: `false`)

### Example

```slint
export component Example inherits Window {
    width: 270px;
    height: 100px;

    Rectangle {
        x: 10px;
        y: 10px;
        width: 50px;
        height: 50px;
        background: blue;
    }

    // Rectangle with a border
    Rectangle {
        x: 70px;
        y: 10px;
        width: 50px;
        height: 50px;
        background: green;
        border-width: 2px;
        border-color: red;
    }

    // Transparent Rectangle with a border and a radius
    Rectangle {
        x: 140px;
        y: 10px;
        width: 50px;
        height: 50px;
        border-width: 4px;
        border-color: black;
        border-radius: 10px;
    }

    // A radius of width/2 makes it a circle
    Rectangle {
        x: 210px;
        y: 10px;
        width: 50px;
        height: 50px;
        background: yellow;
        border-width: 2px;
        border-color: blue;
        border-radius: self.width/2;
    }
}
```

## `TextInput`

The `TextInput` is a lower-level item that shows text and allows entering text.

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

### Properties

-   **`color`** (_in_ _brush_): The color of the text (default value: depends on the style)
-   **`font-family`** (_in_ _string_): The name of the font family selected for rendering the text.
-   **`font-size`** (_in_ _length_): The font size of the text.
-   **`font-weight`** (_in_ _int_): The weight of the font. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
-   **`font-italic`** (_in_ _bool_): Whether or not the font face should be drawn italicized or not. (default value: false)   
-   **`has-focus`** (_out_ _bool_): `TextInput` sets this to `true` when it's focused. Only then it receives [`KeyEvent`](structs.md#keyevent)s.
-   **`horizontal-alignment`** (_in_ _enum [`TextHorizontalAlignment`](enums.md#texthorizontalalignment)_): The horizontal alignment of the text.
-   **`input-type`** (_in_ _enum [`InputType`](enums.md#inputtype)_): Use this to configure `TextInput` for editing special input, such as password fields. (default value: `text`)
-   **`letter-spacing`** (_in_ _length_): The letter spacing allows changing the spacing between the glyphs. A positive value increases the spacing and a negative value decreases the distance. (default value: 0)
-   **`read-only`** (_in_ _bool_): When set to `true`, text editing via keyboard and mouse is disabled but selecting text is still enabled as well as editing text programatically. (default value: `false`)
-   **`selection-background-color`** (_in_ _color_): The background color of the selection.
-   **`selection-foreground-color`** (_in_ _color_): The foreground color of the selection.
-   **`single-line`** (_in_ _bool_): When set to `true`, the text is always rendered as a single line, regardless of new line separators in the text. (default value: `true`)
-   **`text-cursor-width`** (_in_ _length_): The width of the text cursor. (default value: provided at run-time by the selected widget style)
-   **`text`** (_in-out_ _string_): The text rendered and editable by the user.
-   **`vertical-alignment`** (_in_ _enum [`TextVerticalAlignment`](enums.md#textverticalalignment)_): The vertical alignment of the text.
-   **`wrap`** (_in_ _enum [`TextWrap`](enums.md#textwrap)_): The way the text input wraps. Only makes sense when `single-line` is false. (default value: no-wrap)

### Functions

-   **`focus()`** Call this function to focus the text input and make it receive future keyboard events.
-   **`select-all()`** Selects all text.
-   **`copy()`** Copies the selected text to the clipboard.
-   **`cut()`** Copies the selected text to the clipboard and removes it from the editable area.
-   **`paste()`** Pastes the text content of the clipboard at the cursor position.

### Callbacks

-   **`accepted()`**: Invoked when enter key is pressed.
-   **`cursor-position-changed(`[_`Point`_](structs.md#point)`)`**: The cursor was moved to the new (x, y) position.
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

## `Text`

The `Text` element is responsible for rendering text. Besides the `text` property, that specifies which text to render,
it also allows configuring different visual aspects through the `font-family`, `font-size`, `font-weight` and `color` properties.

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
-   **`horizontal-alignment`** (_in_ _enum [`TextHorizontalAlignment`](enums.md#texthorizontalalignment)_): The horizontal alignment of the text.
-   **`letter-spacing`** (_in_ _length_): The letter spacing allows changing the spacing between the glyphs. A positive value increases the spacing and a negative value decreases the distance. (default value: 0)
-   **`overflow`** (_in_ _enum [`TextOverflow`](enums.md#textoverflow)_): What happens when the text overflows (default value: clip).
-   **`text`** (_in_ _[string](../reference/types.md#strings)_): The text rendered.
-   **`vertical-alignment`** (_in_ _enum [`TextVerticalAlignment`](enums.md#textverticalalignment)_): The vertical alignment of the text.
-   **`wrap`** (_in_ _enum [`TextWrap`](enums.md#textwrap)_): The way the text wraps (default value: `no-wrap`).

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

## `TouchArea`

Use `TouchArea` to control what happens when the region it covers is touched or interacted with
using the mouse.

When not part of a layout, its width or height default to 100% of the parent element.

### Properties

-   **`has-hover`** (_out_ _bool_): `TouchArea` sets this to `true` when the mouse is over it.
-   **`mouse-cursor`** (_in_ _enum [`MouseCursor`](enums.md#mousecursor)_): The mouse cursor type when the mouse is hovering the `TouchArea`.
-   **`mouse-x`**, **`mouse-y`** (_out_ _length_): Set by the `TouchArea` to the position of the mouse within it.
-   **`pressed-x`**, **`pressed-y`** (_out_ _length_): Set by the `TouchArea` to the position of the mouse at the moment it was last pressed.
-   **`pressed`** (_out_ _bool_): Set to `true` by the `TouchArea` when the mouse is pressed over it.

### Callbacks

-   **`clicked()`**: Invoked when clicked: The mouse is pressed, then released on this element.
-   **`moved()`**: The mouse has been moved. This will only be called if the mouse is also pressed.
-   **`pointer-event(`[_`PointerEvent`_](structs.md#pointerevent)`)`**: Invoked when a button was pressed or released.

### Example

```slint
export component Example inherits Window {
    width: 200px;
    height: 100px;
    area := TouchArea {
        width: parent.width;
        height: parent.height;
        clicked => {
            rect2.background = #ff0;
        }
    }
    Rectangle {
        x:0;
        width: parent.width / 2;
        height: parent.height;
        background: area.pressed ? blue: red;
    }
    rect2 := Rectangle {
        x: parent.width / 2;
        width: parent.width / 2;
        height: parent.height;
    }
}
```

## `VerticalLayout` and `HorizontalLayout`

These layouts place their children next to each other vertically or horizontally.
The size of elements can either be fixed with the `width` or `height` property, or if they aren't set
they will be computed by the layout respecting the minimum and maximum sizes and the stretch factor.

### Properties

-   **`spacing`** (_in_ _length_): The distance between the elements in the layout.
-   **`padding`** (_in_ _length_): the padding within the layout.
-   **`padding-left`**, **`padding-right`**, **`padding-top`** and **`padding-bottom`** (_in_ _length_): Set these properties to override the padding on specific sides.
-   **`alignment`** (_in_ _enum [`LayoutAlignment`](enums.md#layoutalignment)_): Set the alignment. Matches the CSS flex box.

### Example

```slint
export component Foo inherits Window {
    width: 200px;
    height: 100px;
    HorizontalLayout {
        spacing: 5px;
        Rectangle { background: red; width: 10px; }
        Rectangle { background: blue; min-width: 10px; }
        Rectangle { background: yellow; horizontal-stretch: 1; }
        Rectangle { background: green; horizontal-stretch: 2; }
    }
}
```

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
-   **`title`** (_in_ _string_): The window title that is shown in the title bar.
