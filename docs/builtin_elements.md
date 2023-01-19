# Builtin Elements

## Common properties

These properties are valid on all visible items

* **`x`** and **`y`** (*length*): the position of the element relative to its parent
* **`z`** (*float*): Allows to specify a different order to stack the items with its siblings. (default: 0)
* **`width`** and **`height`** (*length*): The size of the element. When set, this overrides the default size.
* **`max-width`** and **`max-height`** (*length*): The maximum size of an element when used in a layout.
* **`min-width`** and **`min-height`** (*length*): The minimum size of an element when used in a layout.
* **`preferred-width`** and **`preferred-height`** (*length*): The preferred size of an element when used in a layout.
* **`col`**, **`row`**, **`colspan`**, **`rowspan`** (*int*): See [`GridLayout`](#gridlayout).
* **`horizontal-stretch`** and **`vertical-stretch`** (*float*): Specify how much relative space these elements are stretching in a layout.
  When 0, this means that the elements will not be stretched unless all elements are 0. Builtin widgets have a value of either 0 or 1
* **`opacity`** (*float*): A value between 0 and 1 (or a percentage) that is used to draw the element and its
  children with transparency. 0 is fully transparent (invisible), and 1 is fully opaque. (default: 1)
* **`visible`** (*bool*): When set to `false`, the element and all his children will not be drawn
  and not react to mouse input (default: `true`)
* **`cache-rendering-hint`** (*bool*): When set to `true`, this provides a hint
  to the renderer to cache the contents of the element and all the children into an intermediate
  cached layer. For complex sub-trees that rarely change this may speed up the rendering, at the
  expense of increased memory consumption. Not all rendering backends support this, so this is
  merely a hint. (default: `false`)
* **`dialog-button-role`** (*enum DialogButtonRole*): Specify that this is a button in a `Dialog`.

### Accessibility

Use the following `accessible-` properties to make your items interact well with software like screen readers, braille terminals and other software to make your application accessible.

* **`accessible-role`** (*enum [`AccessibleRole`](builtin_enums.md#accessiblerole)*): The accessibility role of the element. This property is
  mandatory to be able to use any other accessible properties. It should be set to a constant value.
  The default value is `none` for most elements, but is `text` for the Text element.
* **`accessible-label`** (*string*): The label for an interactive element.
  The  default value is empty for most elements, or is the value of the `text` property for Text
  elements.
* **`accessible-description`** (*string*): The description for the current element.
* **`accessible-checkable`** (*bool*): Whether the element is can be checked or not.
* **`accessible-checked`** (*bool*): Whether the element is checked or not. This maps to the "checked" state of checkboxes, radio buttons, and other widgets.
* **`accessible-has-focus`** (*bool*): Set to true when the current element currently has the focus.
* **`accessible-value`** (*string*): The current value of the item.
* **`accessible-value-maximum`** (*float*): The maximum value of the item. This is used for example by spin boxes.
* **`accessible-value-minimum`** (*float*): The minimum value of the item.
* **`accessible-value-step`** (*float*) The smallest increment or decrement by which the current value can change. This corresponds to the step by which a handle on a slider can be dragged.

### Drop Shadows

To achieve the graphical effect of a visually elevated shape that shows a shadow effect underneath the frame of
an element, it is possible to set the following `drop-shadow` properties:

* **`drop-shadow-offset-x`** and **`drop-shadow-offset-y`** (*length*): The horizontal and vertical distance
  of the shadow from the element's frame. A negative value places the shadow left / above of the element.
* **`drop-shadow-color`** (*color*): The base color of the shadow to use. Typically that color is the starting color
  of a gradient that fades into transparency.
* **`drop-shadow-blur`** (*length*): The radius of the shadow that also describes the level of blur applied to the shadow.
  Negative values are ignored and zero means no blur (default).

The `drop-shadow` effect is supported for `Rectangle` elements.

## `Window`

Window is the root of what is on the screen

The Window geometry will be restricted by its layout constraints: setting the `width` will result in a fixed width,
and the window manager will respect the `min-width` and `max-width` so the window can't be resized bigger
or smaller. The initial width can be controlled with the `preferred-width` property. The same applies for the height.

### Properties

* **`title`** (*string*): The window title that is shown in the title bar.
* **`icon`** (*image*): The window icon shown in the title bar or the task bar on window managers supporting it.
* **`no-frame`** (*bool*): Whether the window should be borderless/frameless or not.
* **`background`** (*color*): The background color of the Window. (default value: depends on the style)
* **`default-font-family`** (*string*): The font family to use as default in text elements inside this window, that don't
  have their family set.
* **`default-font-size`** (*length*): The font size to use as default in text elements inside this window, that don't
  have their size set.
* **`default-font-weight`** (*int*): The font weight to use as default in text elements inside this window, that don't
  have their weight set. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.

## `Rectangle`

By default, the rectangle is just an empty item that shows nothing. By setting a color or a border
it is then possible to draw a simple rectangle on the screen

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

### Properties

* **`background`** (*brush*): The background brush of the Rectangle, typically a color. (default value: transparent)
* **`border-width`** (*length*): The width of the border. (default value: 0)
* **`border-color`** (*brush*): The color of the border. (default value: transparent)
* **`border-radius`** (*length*): The size of the radius. (default value: 0)
* **`clip`** (*bool*): By default, when an item is bigger or outside another item, it is still shown.
  But when this property is set to `true`, then the children element of this Rectangle are going
  to be clipped. (default: `false`)

### Example

```slint
component Example inherits Window {
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

## `Image`

An Image can be used to represent an image loaded from an image file.

### Properties

* **`source`** (*image*): The image to load. In order to reference image, one uses the `@image-url("...")` macro
  which loads the file relative to the directory containing the .slint file.
* **`source-clip-x`**, **`source-clip-y`**, **`source-clip-width`**, **`source-clip-height`** (*int*): Properties in source
  image coordinates that define the region of the source image that is rendered. By default the entire source image is visible:
  | Property | Default Binding |
  |----------|---------------|
  | `source-clip-x` | `0` |
  | `source-clip-y` | `0` |
  | `source-clip-width` | `source.width - source-clip-x` |
  | `source-clip-height` | `source.height - source-clip-y` |
* **`image-fit`** (*enum*): Specifies how the source image shall be fit into the image element. Possible values are:
  * `fill`: Scales and stretches the image to fit the width and height of the element.
  * `contain`: The source image is scaled to fit into the image element's dimension while preserving the aspect ratio.
  * `cover`: The source image is scaled to cover into the image element's dimension while preserving the aspect ratio.

  When the `Image` element is part of a layout, the default value for **`image-fit`** is `contain`. Otherwise it is `fill`.

* **`image-rendering`** (*enum*): Specifies how the source image will be scaled. Possible values are:
  * `smooth`: The image is scaled with a linear interpolation algorithm.
  * `pixelated`: The image is scaled with the nearest neighbor algorithm.

  The default value is `smooth`.

* **`colorize`** (*brush*): When set, the image is used as an alpha mask and is drown in the given color (or with the gradient)
* **`width`**, **`height`** (*length*): The width and height of the image as it appears on the screen.The default values are
  the sizes provided by the **`source`** image. If the `Image` is **not** in a layout and only **one** of the two sizes are
  specified, then the other defaults to the specified value scaled according to the aspect ratio of the **`source`** image.

* **`rotation-angle`** (*angle*), **`rotation-origin-x`** (*length*), **`rotation-origin-y`** (*length*):
  Rotate the image by the given angle around the specified origin point. The default origin point is the center of the element.
  When these properties are present, the Image cannot have any children elements.

### Example

```slint
component Example inherits Window {
    width: 100px;
    height: 100px;
    VerticalLayout {
        Image {
            source: @image-url("https://slint-ui.com/logo/slint-logo-full-light.svg");
            // image-fit default is `contain` when in layout, preserving aspect ratio
        }
        Image {
            source: @image-url("https://slint-ui.com/logo/slint-logo-full-light.svg");
            colorize: red;
        }
    }
}
```

Scaled while preserving the aspect ratio:

```slint
component Example inherits Window {
    width: 100px;
    height: 150px;
    VerticalLayout {
        Image {
            source: @image-url("https://slint-ui.com/logo/slint-logo-full-light.svg");
            width: 100px;
            // implicit default, preserving aspect ratio:
            // height: self.width * natural_height / natural_width;
        }
    }
}
```

## `Text`

The `Text` element is responsible for rendering text. Besides the `text` property, that specifies which text to render,
it also allows configuring different visual aspects through the `font-family`, `font-size`, `font-weight` and `color` properties.

The `Text` element can break long text into multiple lines of text. A line feed character (`\n`) in the string of the `text`
property will trigger a manual line break. For automatic line breaking you need to set the `wrap` property to a value other than
`no-wrap` and it is important to specify a `width` and `height` for the `Text` element, in order to know where to break. It's
recommended to place the `Text` element in a layout and let it set the `width` and `height` based on the available screen space
and the text itself.

### Properties

* **`text`** (*string*): The actual text.
* **`font-family`** (*string*): The font name
* **`font-size`** (*length*): The font size of the text
* **`font-weight`** (*int*): The weight of the font. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
* **`color`** (*brush*): The color of the text (default value: depends on the style)
* **`horizontal-alignment`** (*enum [`TextHorizontalAlignment`](builtin_enums.md#texthorizontalalignment)*): The horizontal alignment of the text.
* **`vertical-alignment`** (*enum [`TextVerticalAlignment`](builtin_enums.md#textverticalalignment)*): The vertical alignment of the text.
* **`wrap`** (*enum [`TextWrap`](builtin_enums.md#textwrap)*): The way the text wraps (default: no-wrap).
* **`overflow`** (*enum [`TextOverflow`](builtin_enums.md#textoverflow)*): What happens when the text overflows (default: clip).
* **`letter-spacing`** (*length*): The letter spacing allows changing the spacing between the glyphs. A positive value increases the spacing
  and a negative value decreases the distance. The default value is 0.

### Example

This example shows the text "Hello World" in red, using the default font:

```slint
component Example inherits Window {
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
component Example inherits Window {
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

## `Path`

The `Path` element allows rendering a generic shape, composed of different geometric commands. A path
shape can be filled and outlined.

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

A path can be defined in two different ways:

* Using SVG path commands as a string
* Using path command elements in `.slint` markup.

The coordinates used in the geometric commands are within the imaginary coordinate system of the path.
When rendering on the screen, the shape is drawn relative to the `x` and `y` properties. If the `width`
and `height` properties are non-zero, then the entire shape is fit into these bounds - by scaling
accordingly.

### Common Path Properties

* **`fill`** (*brush*): The color for filling the shape of the path.
* **`fill-rule`** (*enum [`FillRule`](builtin_enums.md#fillrule)*): The fill rule to use for the path. (default value: `nonzero`)
* **`stroke`** (*brush*): The color for drawing the outline of the path.
* **`stroke-width`** (*length*): The width of the outline.
* **`width`** (*length*): If non-zero, the path will be scaled to fit into the specified width.
* **`height`** (*length*): If non-zero, the path will be scaled to fit into the specified height.
* **`viewbox-x`**/**`viewbox-y`**/**`viewbox-width`**/**`viewbox-height`** (*float*) These four
  properties allow defining the position and size of the viewport of the path in path coordinates.

  If the `viewbox-width` or `viewbox-height` is less or equal than zero, the viewbox properties are
  ignored and instead the bounding rectangle of all path elements is used to define the view port.
* **`clip`** (*bool*): By default, when a path has a view box defined and the elements render
  outside of it, they are still rendered. When this property is set to `true`, then rendering will be
  clipped at the boundaries of the view box.
  This property must be a literal `true` or `false` (default: `false`)

#### Path Using SVG commands

SVG is a popular file format for defining scalable graphics, which are often composed of paths. In SVG
paths are composed using [commands](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/d#path_commands),
which in turn are written in a string. In `.slint` the path commands are provided to the `commands`
property. The following example renders a shape consists of an arc and a rectangle, composed of `line-to`,
`move-to` and `arc` commands:

```slint
component Example inherits Path {
    width: 100px;
    height: 100px;
    commands: "M 0 0 L 0 100 A 1 1 0 0 0 100 100 L 100 0 Z";
    stroke: red;
    stroke-width: 1px;
}
```

The commands are provided in a property:

* **`commands`** (*string): A string providing the commands according to the SVG path specification.

#### Path Using SVG Path Elements

The shape of the path can also be described using elements that resemble the SVG path commands but use the
`.slint` markup syntax. The earlier example using SVG commands can also be written like that:

```slint
component Example inherits Path {
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

Note how the coordinates of the path elements do not use units - they operate within the imaginary
coordinate system of the scalable path.

##### `MoveTo` Sub-element for `Path`

The `MoveTo` sub-element closes the current sub-path, if present, and moves the current point
to the location specified by the `x` and `y` properties. Subsequent elements such as `LineTo`
will use this new position as their starting point, therefore this starts a new sub-path.

###### Properties

* **`x`** (*float*): The x position of the new current point.
* **`y`** (*float*): The y position of the new current point.

##### `LineTo` Sub-element for `Path`

The `LineTo` sub-element describes a line from the path's current position to the
location specified by the `x` and `y` properties.

###### Properties

* **`x`** (*float*): The target x position of the line.
* **`y`** (*float*): The target y position of the line.

##### `ArcTo` Sub-element for `Path`

The `ArcTo` sub-element describes the portion of an ellipse. The arc is drawn from the path's
current position to the location specified by the `x` and `y` properties. The remaining properties
are modelled after the SVG specification and allow tuning visual features such as the direction
or angle.

###### Properties

* **`x`** (*float*): The target x position of the line.
* **`y`** (*float*): The target y position of the line.
* **`radius-x`** (*float*): The x-radius of the ellipse.
* **`radius-y`** (*float*): The y-radius of the ellipse.
* **`x-rotation`** (*float*): The x-axis of the ellipse will be rotated by the value of this
  properties, specified in as angle in degrees from 0 to 360.
* **`large-arc`** (*bool*): Out of the two arcs of a closed ellipse, this flag selects that the
  larger arc is to be rendered. If the property is `false`, the shorter arc is rendered instead.
* **`sweep`** (*bool*): If the property is `true`, the arc will be drawn as a clockwise turning arc;
  anti-clockwise otherwise.

##### `CubicTo` Sub-element for `Path`

The `CubicTo` sub-element describes a smooth Bézier from the path's current position to the
location specified by the `x` and `y` properties, using two control points specified by their
respective properties.

###### Properties

* **`x`** (*float*): The target x position of the curve.
* **`y`** (*float*): The target y position of the curve.
* **`control-1-x`** (*float*): The x coordinate of the curve's first control point.
* **`control-1-y`** (*float*): The y coordinate of the curve's first control point.
* **`control-2-x`** (*float*): The x coordinate of the curve's second control point.
* **`control-2-y`** (*float*): The y coordinate of the curve's second control point.

##### `QuadraticTo` Sub-element for `Path`

The `QuadraticTo` sub-element describes a smooth Bézier from the path's current position to the
location specified by the `x` and `y` properties, using the control points specified by the
`control-x` and `control-y` properties.

###### Properties

* **`x`** (*float*): The target x position of the curve.
* **`y`** (*float*): The target y position of the curve.
* **`control-x`** (*float*): The x coordinate of the curve's control point.
* **`control-y`** (*float*): The y coordinate of the curve's control point.

##### `Close` Sub-element for `Path`

The `Close` element closes the current sub-path and draws a straight line from the current
position to the beginning of the path.

## `TouchArea`

The TouchArea control what happens when the zone covered by it is touched or interacted with
using the mouse.

When not part of a layout, its width or height default to 100% of the parent element if not specified.

### Properties

* **`pressed`** (*bool*): Set to `true` by the TouchArea when the mouse is pressed over it.
* **`has-hover`** (*bool*): Set to `true` by the TouchArea when the mouse is over it.
* **`mouse-x`**, **`mouse-y`** (*length*): Set by the TouchArea to the position of the mouse within it.
* **`pressed-x`**, **`pressed-y`** (*length*): Set to `true` by the TouchArea to the position of the mouse at the moment it was last pressed.
* **`mouse-cursor`** (*enum [`MouseCursor`](builtin_enums.md#mousecursor)*): The mouse cursor type when the mouse is hovering the TouchArea.

### Callbacks

* **`clicked`**: Emitted when clicked (the mouse is pressed, then released on this element)
* **`moved`**: The mouse has been moved. This will only be called if the mouse is also pressed.
* **`pointer-event(PointerEvent)`**: Received when a button was pressed or released.

### Example

```slint
component Example inherits Window {
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

## `FocusScope`

The FocusScope exposes callback to intercept the pressed key when it has focus.

The KeyEvent has a text property which is a character of the key entered.
When a non-printable key is pressed, the character will be either a control character,
or it will be mapped to a private unicode character. The mapping of these non-printable, special characters is available in the [`Key`](#key) namespace

### Properties

* **`has-focus`** (*bool*): Set to `true` when item is focused and receives keyboard events.

### Methods

* **`focus()`** Call this function to focus the text input and make it receive future keyboard events.

### Callbacks

* **`key-pressed(KeyEvent) -> EventResult`**: Emitted when a key is pressed, the argument is a `KeyEvent` struct
* **`key-released(KeyEvent) -> EventResult`**: Emitted when a key is released, the argument is a `KeyEvent` struct

### Example

```slint
component Example inherits Window {
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

## `VerticalLayout` / `HorizontalLayout`

These layouts place their children next to each other vertically or horizontally.
The size of elements can either be fixed with the `width` or `height` property, or if they are not set
they will be computed by the layout respecting the minimum and maximum sizes and the stretch factor.

## Properties

* **`spacing`** (*length*): The distance between the elements in the layout.
* **`padding`** (*length*): the padding within the layout.
* **`padding-left`**, **`padding-right`**, **`padding-top`** and **`padding-bottom`** (*length*):
  override the padding in specific sides.
* **`alignment`** (*FIXME enum*): Can be one of  `stretch`, `center`, `start`, `end`,
  `space-between`, `space-around`. Defaults to `stretch`. Matches the CSS flex.

## Example

```slint
component Foo inherits Window {
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

## `GridLayout`

`GridLayout` places the elements in a grid. `GridLayout` adds properties to each item: `col`, `row`, `colspan`, `rowspan`.
You can control the position of elements with `col` and `row`.
If `col` or `row` is not specified, they are automatically computed such that the item is next to the previous item, in the same row.
Alternatively, the item can be put in a `Row` element.

### Properties

* **`spacing`** (*length*): The distance between the elements in the layout.
* **`padding`** (*length*): the padding within the layout.
* **`padding-left`**, **`padding-right`**, **`padding-top`** and **`padding-bottom`** (*length*):
  override the padding in specific sides.

### Examples

This example uses the `Row` element

```slint
component Foo inherits Window {
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
component Foo inherits Window {
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

## `Flickable`

The `Flickable` is a lower-level item that is the base for scrollable
elements, such as the ScrollView widget. When the `viewport-width` or the
`viewport-height` is greater than the parent width or parent height
respectively the element becomes scrollable although the `Flickable`
does not create a scrollbar. When unset, the `viewport-width` and `viewport-height` are
calculated automatically based on the content. Excepted when using a `for` loop to populate
the elements, that is tracked in issue #407.
The maximum and preferred size of the Flickable are based on those of the viewport.

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

### Properties

* **`viewport-height`**, **`viewport-width`** (*length*): The total size of the scrollable element
* **`viewport-x`**, **`viewport-y`** (*length*): The position of the scrollable element relative to the Flickable.  This is usually a negative value.
* **`interactive`** (*bool*): When true, the viewport can be scrolled by clicking on it and dragging it with the cursor. (default: true)

### Example

```slint
component Example inherits Window {
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

## `TextInput`

The `TextInput` is a lower-level item that shows text and allows entering text.

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

### Properties

* **`text`** (*string*): The actual text.
* **`font-family`** (*string*): The font name
* **`font-size`** (*length*): The font size of the text
* **`font-weight`** (*int*): The weight of the font. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
* **`color`** (*brush*): The color of the text (default value: depends on the style)
* **`horizontal-alignment`** (*enum [`TextHorizontalAlignment`](builtin_enums.md#texthorizontalalignment)*): The horizontal alignment of the text.
* **`vertical-alignment`** (*enum [`TextVerticalAlignment`](builtin_enums.md#textverticalalignment)*): The vertical alignment of the text.
* **`has-focus`** (*bool*): Set to `true` when item is focused and receives keyboard events.
* **`letter-spacing`** (*length*): The letter spacing allows changing the spacing between the glyphs. A positive value increases the spacing
  and a negative value decreases the distance. The default value is 0.
* **`single-line`** (*bool*): When set to `true`, no newlines are allowed (default value: `true`)
* **`read-only`** (*bool*): When set to `true`, text editing via keyboard and mouse is disabled but
  selecting text is still enabled as well as editing text programatically (default value: `false`)
* **`wrap`** (*enum [`TextWrap`](builtin_enums.md#textwrap)*): The way the text input wraps.  Only makes sense when `single-line` is false. (default: no-wrap)
* **`input-type`** (*enum [`InputType`](builtin_enums.md#InputType)*): The way to allow special input viewing properties such as password fields (default value: `text`).

### Methods

* **`focus()`** Call this function to focus the text input and make it receive future keyboard events.

### Callbacks

* **`accepted()`**: Emitted when enter key is pressed
* **`edited()`**: Emitted when the text has changed because the user modified it
* **`cursor-position-changed(Point)`**: The cursor was moved to the new (x, y) position

### Example

```slint
component Example inherits Window {
    width: 270px;
    height: 100px;

    TextInput {
        text: "Replace me with a name";
    }
}
```

## `PopupWindow`

This allow to show a popup window like a tooltip or a popup menu.

Note: It is not allowed to access properties of elements within the popup from outside of the popup.

### Methods

* **`show()`** Call this function to show the popup.

### Example

```slint
component Example inherits Window {
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

## `Dialog`

Dialog is like a window, but it has buttons that are automatically laid out.

A Dialog should have one main element for the content, that is not a button.
And the window can have any number of `StandardButton` widgets or other button
with the `dialog-button-role` property.
The button will be layed out in an order that depends on the platform.

The `kind` property of the `StandardButton`s and the `dialog-button-role` properties needs to be set to a specific value,
it cannot be a complex expression.
There cannot be several StandardButton of the same kind.

A callback `<kind>_clicked` is automatically added for each StandardButton which does not have an explicit
callback handler, so it can be handled from the native code. (e.g. if there is a button of kind `cancel`,
a `cancel_clicked` callback will be added).

When viewed with the `slint-viewer` program, the `ok`, `cancel`, and `close` button will cause the dialog to close.

### Properties

* **`title`** (*string*): The window title that is shown in the title bar.
* **`icon`** (*image*): The window icon shown in the title bar or the task bar on window managers supporting it.

### Example

```slint
import { StandardButton, Button } from "std-widgets.slint";
component Example inherits Dialog {
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

# Builtin Structures

## `Point`

This structure represents a point with x and y coordinate

### Fields

* **`x`** (*length*)
* **`y`** (*length*)

## `KeyEvent`

This structure is generated and passed to the key press and release
callbacks of the `FocusScope` element.

### Fields

* **`text`** (*string*): The string representation of the key
* **`modifiers`** (*KeyboardModifiers*): The keyboard modifiers pressed during the event

## `KeyboardModifiers`

This structure is generated as part of `KeyEvent`, to indicate which modifier keys
are pressed during the generation of a key event.

### Fields

* **`control`** (*bool*): `true` if the control key is pressed. On macOS this corresponds to the command key.
* **`alt`** (*bool*): `true` if alt key is pressed.
* **`shift`** (*bool*): `true` if the shift key is pressed.
* **`meta`** (*bool*): `true` if the windows key is pressed on Windows, or the control key on macOS.

## `PointerEvent`

This structure is generated and passed to the `pointer-event` callback of the `TouchArea` element.

### Fields

* **`kind`** (*enum PointerEventKind*): The kind of the event: one of the following
   - `down`: The button was pressed.
   - `up`: The button was released.
   - `cancel`: Another element or window took hold of the grab. This applies to all pressed button and the `button` is not relevent.
* **`button`** (*enum PointerEventButton*): The button that was pressed or released. `left`, `right`, `middle`, or `none`.

# Namespaces

The following namespaces provide access to common constants such as special keys or named colors.

## `Key`

Use the constants in the `Key` namespace to handle pressing of keys that don't have a printable character. Check the value of [`KeyEvent`](#keyevent)'s `text` property
against the constants below.

* **`Backspace`**
* **`Tab`**
* **`Return`**
* **`Escape`**
* **`Backtab`**
* **`Delete`**
* **`Shift`**
* **`Control`**
* **`Alt`**
* **`AltGr`**
* **`CapsLock`**
* **`ShiftR`**
* **`ControlR`**
* **`Meta`**
* **`MetaR`**
* **`UpArrow`**
* **`DownArrow`**
* **`LeftArrow`**
* **`RightArrow`**
* **`F1`**
* **`F2`**
* **`F3`**
* **`F4`**
* **`F5`**
* **`F6`**
* **`F7`**
* **`F8`**
* **`F9`**
* **`F10`**
* **`F11`**
* **`F12`**
* **`F13`**
* **`F14`**
* **`F15`**
* **`F16`**
* **`F17`**
* **`F18`**
* **`F19`**
* **`F20`**
* **`F21`**
* **`F22`**
* **`F23`**
* **`F24`**
* **`Insert`**
* **`Home`**
* **`End`**
* **`PageUp`**
* **`PageDown`**
* **`ScrollLock`**
* **`Pause`**
* **`SysReq`**
* **`Stop`**
* **`Menu`**

## `Colors`

Use the colors namespace to select colors by their name. For example you can use `Colors.aquamarine` or `Colors.bisque`.
The entire list of names is very long. You can find a complete list in the [CSS Specification](https://www.w3.org/TR/css-color-3/#svg-color).
