# Builtin Elements

## Common properties

These properties are valid on all visible items

* **`x`** and **`y`** (*length*): the position of the element relative to its parent
* **`width`** and **`height`** (*length*): The size of the element. When set, this overrides the default size.
* **`maximum_width`** and **`maximum_height`** (*length*): The maximum size of an element when used in a layout.
* **`minimum_width`** and **`minimum_height`** (*length*): The minimum size of an element when used in a layout.
* **`col`**, **`row`**, **`colspan`**, **`rowspan`** (*int*): See [`GridLayout`](#gridlayout).
* **`horizontal_stretch`** and **`vertical_stretch`** (*float*): Specify how much relative space these elements are stretching in a layout.
  When 0, this means that the elements will not be stretched unless all elements are 0. Builtin widgets have a value of either 0 or 1

 ### Drop Shadows

To achieve the graphical effect of a visually elevated shape that shows a shadow effect underneath the frame of
an element, it is possible to set the following `drop-shadow` properties:

* **`drop-shadow-offset-x`** and **`drop-shadow-offset-y`** (*length*): The horizontal and vertical distance of the
  of the shadow from the element's frame. A negative value places the shadow left / above of the element.
* **`drop-shadow-color`** (*color*): The base color of the shadow to use. Typically that color is the starting color
  of a gradient that fades into transparency.
* **`drop-shadow-blur`** (*length*): The size of the blurred area, over which the shadow color is drawn, possibly shaded.

The `drop-shadow` effect is supported for `Rectangle` and `Clip` elements.

## `Window`

Window is the root of what is on the screen

### Properties

* **`title`** (*string*): The window title that is shown in the title bar.
* **`background`** (*color*): The background color of the Window. (default value: depends on the style)

## `Rectangle`

By default, the rectangle is just an empty item that shows nothing. By setting a color or a border
it is then possible to draw a simple rectangle on the screen

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

### Properties

* **`background`** (*brush*): The background brush of the Rectangle, typically a color. (default value: transparent)
* **`border_width`** (*length*): The width of the border. (default value: 0)
* **`border_color`** (*brush*): The color of the border. (default value: transparent)
* **`border_radius`** (*length*): The size of the radius. (default value: 0)

### Example

```60
Example := Window {
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
        border_width: 2px;
        border_color: red;
    }

    // Transparent Rectangle with a border and a radius
    Rectangle {
        x: 140px;
        y: 10px;
        width: 50px;
        height: 50px;
        border_width: 4px;
        border_color: black;
        border_radius: 10px;
    }

    // A radius of width/2 makes it a circle
    Rectangle {
        x: 210px;
        y: 10px;
        width: 50px;
        height: 50px;
        background: yellow;
        border_width: 2px;
        border_color: blue;
        border_radius: width/2;
    }
}
```

## `Image`

An Image can be used to represent an image loaded from an image file

### Properties

* **`source`** (*image*): The image to load. In order to reference image, one uses the `@image-url("...")` macro
  which loads the file relative to the directory containing the .60 file.
* **`source-clip-x`**, **`source-clip-y`**, **`source-clip-width`**, **`source-clip-height`** (*int*): properties in source
  image coordinates that, when specified, can be used to render only a portion of the specified image.
* **`image-fit`** (*enum*): Specifies how the source image shall be fit into the image element. Possible values are:
   * `fill` (default): Scales and stretches the image to fit the width and height of the element.
   * `contain`: The source image is scaled to fit into the image element's dimension while preserving the aspect ratio.
   * `cover`: The source image is scaled to cover into the image element's dimension while preserving the aspect ratio.
* **`colorize`** (*brush*): When set, the image is used as an alpha mask and is drown in the given color (or with the gradient)

### Example


```60
Example := Window {
    VerticalLayout {
        Image {
            source: @image-url("https://sixtyfps.io/resources/logo_scaled.png");
        }
        Image {
            source: @image-url("https://sixtyfps.io/resources/logo_scaled.png");
            colorize: red;
        }
    }
}
```

## `Text`

A text simply show the text on the screen

### Properties

* **`text`** (*string*): The actual text.
* **`font_family`** (*string*): The font name
* **`font_size`** (*length*): The font size of the text
* **`font_weight`** (*int*): The weight of the font. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
* **`color`** (*brush*): The color of the text (default: black)
* **`horizontal_alignment`** (*enum [`TextHorizontalAlignment`](#texthorizontalalignment)*): The horizontal alignment of the text.
* **`vertical_alignment`** (*enum [`TextVerticalAlignment`](#textverticalalignment)*): The vertical alignment of the text.
* **`wrap`** (*enum [`TextWrap`](#textwrap)*): The way the text wraps (default: no-wrap).
* **`overflow`** (*enum [`TextOverflow`](#textoverflow)*): What happens when the text overflows (default: clip).
* **`letter_spacing`** (*length*): The letter spacing allows changing the spacing between the glyphs. A positive value increases the spacing
  and a negative value decreases the distance. The default value is 0.


### Example

```60
Example := Window {
    width: 270px;
    height: 100px;

    Text {
        text: "Hello World";
        color: red;
    }
}
```

## `Path`

The `Path` element allows rendering a generic shape, composed of different geometric commands. A path
shape can be filled and outlined.

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

A path can be defined in two different ways:

  * Using SVG path commands as a string
  * Using path command elements in `.60` markup.

The coordinates used in the geometric commands are within the imaginary coordinate system of the path.
When rendering on the screen, the shape is drawn relative to the `x` and `y` properties. If the `width`
and `height` properties are non-zero, then the entire shape is fit into these bounds - by scaling
accordingly.

### Common Path Properties

* **`fill`** (*brush*): The color for filling the shape of the path.
* **`fill-rule`** (enum *[`FillRule`](#fillrule)*): The fill rule to use for the path. (default value: `nonzero`)
* **`stroke`** (*brush*): The color for drawing the outline of the path.
* **`stroke-width`** (*length*): The width of the outline.
* **`width`** (*length*): If non-zero, the path will be scaled to fit into the specified width.
* **`height`** (*length*): If non-zero, the path will be scaled to fit into the specified height.

#### Path Using SVG commands

SVG is a popular file format for defining scalable graphics, which are often composed of paths. In SVG
paths are composed using [commands](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/d#path_commands),
which in turn are written in a string literal. In `.60` the path commands are provided to the `commands`
property. The following example renders a shape consists of an arc and a rectangle, composed of `line-to`,
`move-to` and `arc` commands:

```60
Example := Path {
    width: 100px;
    height: 100px;
    commands: "M 0 0 L 0 100 A 1 1 0 0 0 100 100 L 100 0 Z";
    stroke: red;
    stroke-width: 1px;
}
```

The commands are provided in a property:

* **`commands`** (*string): A string literal providing the commands according to the SVG path specification.

#### Path Using SVG Path Elements

The shape of the path can also be described using elements that resemble the SVG path commands but use the
`.60` markup syntax. The earlier example using SVG commands can also be written like that:

```60
Example := Path {
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
will it as their starting point, therefore this starts a new sub-path.

###### Properties

* **`x`** (*float): The x position of the new current point.
* **`y`** (*float): The y position of the new current point.

##### `LineTo` Sub-element for `Path`

The `LineTo` sub-element describes a line from the path's current position to the
location specified by the `x` and `y` properties.
###### Properties

* **`x`** (*float): The target x position of the line.
* **`y`** (*float): The target y position of the line.

##### `ArcTo` Sub-element for `Path`

The `ArcTo` sub-element describes the portion of an ellipse. The arc is drawn from the path's
current position to the location specified by the `x` and `y` properties. The remaining properties
are modelled after the SVG specification and allow tuning visual features such as the direction
or angle.

###### Properties

* **`x`** (*float): The target x position of the line.
* **`y`** (*float): The target y position of the line.
* **`radius-x`** (*float): The x-radius of the ellipse.
* **`radius-y`** (*float): The y-radius of the ellipse.
* **`x-rotation`** (*float): The x-axis of the ellipse will be rotated by the value of this
  properties, specified in as angle in degress from 0 to 360.
* **`large-arc`** (*bool): Out of the two arcs of a closed ellipse, this flag selects that the
  larger arc is to be rendered. If the property is false, the shorter arc is rendered instead.
* **`sweep`** (*bool): If the property is true, the arc will be drawn as a clockwise turning arc;
  anti-clockwise otherwise.

##### `CubicTo` Sub-element for `Path`

The `CubicTo` sub-element describes a smooth Bézier from the path's current position to the
location specified by the `x` and `y` properties, using two control points specified by their
respective properties.
###### Properties

* **`x`** (*float): The target x position of the curve.
* **`y`** (*float): The target y position of the curve.
* **`control-1-x`** (*float): The x coordinate of the curve's first control point.
* **`control-1-y`** (*float): The y coordinate of the curve's first control point.
* **`control-2-x`** (*float): The x coordinate of the curve's second control point.
* **`control-2-y`** (*float): The y coordinate of the curve's second control point.

##### `QuadraticTo` Sub-element for `Path`

The `QuadraticTo` sub-element describes a smooth Bézier from the path's current position to the
location specified by the `x` and `y` properties, using the control points specified by the
`control-x` and `control-y` properties.
###### Properties

* **`x`** (*float): The target x position of the curve.
* **`y`** (*float): The target y position of the curve.
* **`control-x`** (*float): The x coordinate of the curve's control point.
* **`control-y`** (*float): The y coordinate of the curve's control point.

##### `Close` Sub-element for `Path`

The `Close` element closes the current sub-path and draws a straight line from the current
position to the beginning of the path.

## `TouchArea`

The TouchArea control what happens when the zone covered by it is touched or interacted with the mouse.

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

### Properties

* **`pressed`** (*bool*): Set to true by the TouchArea when the mouse is pressed over it.
* **`has_hover`** (*bool*): Set to true by the TouchArea when the mouse is over it.
* **`mouse_x`**, **`mouse_y`** (*length*): Set by the TouchArea to the position of the mouse within it.
* **`pressed_x`**, **`mouse_y`** (*length*): Set to true by the TouchArea to the position of the
    mouse at the moment it was last pressed.

### Callbacks

* **`clicked`**: Emited when the mouse is released

### Example

```60
Example := Window {
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

### Properties

* **`has_focus`** (*bool*): Set to true when item is focused and receives keyboard events.

### Methods

* **`focus()`** Call this function to focus the text input and make it receive future keyboard events.

### Callbacks

* **`key_pressed(KeyEvent) -> EventResult`**: Emited when a key is pressed, the argument is a `KeyEvent` object
* **`key_released(KeyEvent) -> EventResult`**: Emited when a key is released, the argument is a `KeyEvent` object

### Example

```60
Example := Window {
    FocusScope {
        key-pressed(event) => {
            debug(event.text);
            if (event.modifiers.control) {
                debug("control was pressed during this event");
            }
            accept
        }
    }
}
```

## `VerticalLayout` / `HorizontalLayout`

These layouts place their children next to eachother verticaly or horizontally.
The size of elements can either be fixed with the `width` or `height` property, or if they are not set
they will be computed by the layout respecting the minimum and maximum sizes and the strecth factor.

## Properties

 * **`spacing`** (*length*): The distance between the elements in the layout.
 * **`padding`** (*length*): the padding within the layout.
 * **`padding_left`**, **`padding_right`**, **`padding_top`** and **`padding_bottom`** (*length*):
    override the padding in specific sides.
 * **`alignment`** (*FIXME enum*): Can be one of  `stretch`, `center`, `start`, `end`,
    `space_between`, `space_around`. Defaults to `stretch`. Matches the CSS flex.

## Example

```60
Foo := Window {
    width: 200px;
    height: 100px;
    HorizontalLayout {
        spacing: 5px;
        Rectangle { background: red; width: 10px; }
        Rectangle { background: blue; minimum-width: 10px; }
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
 * **`padding_left`**, **`padding_right`**, **`padding_top`** and **`padding_bottom`** (*length*):
    override the padding in specific sides.

### Examples

This example use the `Row` element

```60
Foo := Window {
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

This example use the `col` and `row` property

```60
Foo := Window {
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

## `PathLayout`

FIXME: write docs

## `Flickable`

FIXME: write docs

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

## `TextInput`

The `TextInput` is a lower-level item that shows text and allows entering text.

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

### Properties

* **`text`** (*string*): The actual text.
* **`font_family`** (*string*): The font name
* **`font_size`** (*length*): The font size of the text
* **`font_weight`** (*int*): The weight of the font. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
* **`color`** (*brush*): The color of the text (default: transparent)
* **`horizontal_alignment`** (enum *[`TextHorizontalAlignment`](#texthorizontalalignment)*): The horizontal alignment of the text.
* **`vertical_alignment`** (enum *[`TextVerticalAlignment`](#textverticalalignment)*): The vertical alignment of the text.
* **`has_focus`** (*bool*): Set to true when item is focused and receives keyboard events.
* **`letter_spacing`** (*length*): The letter spacing allows changing the spacing between the glyphs. A positive value increases the spacing
  and a negative value decreases the distance. The default value is 0.

### Methods

* **`focus()`** Call this function to focus the text input and make it receive future keyboard events.

### Callbacks

* **`accepted()`**: Emited when enter key is pressed
* **`edited()`**: Emited when the text has changed because the user modified it

### Example

```60
Example := Window {
    width: 270px;
    height: 100px;

    TextInput {
        text: "Replace me with a name";
    }
}
```

## `Clip`

By default, when an item is bigger or outside another item, it is still shown.
But the `Clip` element  make sure to clip any children outside of the rectangle bounds

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

## `PopupWindow`

This allow to show a popup window like a tooltip or a popup menu.

Note: it is not allowed to access properties on element within the popup from outside of the popup

### Methods

* **`show()`** Call this function to show the popup.

### Example

```60
Example := Window {
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

# Builtin Structures

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

* **`control`** (*bool*): True if the control key is pressed. On macOS this corresponds to the command key.
* **`alt`** (*bool*): True if alt key is pressed.
* **`shift`** (*bool*): True if the shift key is pressed.
* **`meta`** (*bool*): True if the windows key is pressed on Windows, or the control key on macOS.

# Builtin Enums

The default value of each enum type is always the first value

## `TextHorizontalAlignment`

This enum describes the different types of alignment of text along the horizontal axis.

### Values

* **`TextHorizontalAlignment.left`**: The text will be aligned with the left edge of the contained box.
* **`TextHorizontalAlignment.center`**: The text will be horizontally centered within the contained box.
* **`TextHorizontalAlignment.right`** The text will be alignt to the right right of the contained box.

## `TextVerticalAlignment`

This enum describes the different types of alignment of text along the vertical axis.

### Values

* **`TextVerticalAlignment.top`**: The text will be aligned to the top of the contained box.
* **`TextVerticalAlignment.center`**: The text will be vertically centered within the contained box.
* **`TextVerticalAlignment.bottom`** The text will be alignt to the bottom of the contained box.

## `TextWrap`

This enum describes the how the text wrap if it is too wide to fit in the Text width.

### Values

* **`TextWrap.no-wrap`**: The text will not wrap, but instead will overflow.
* **`TextWrap.word-wrap`**: The text will be wrapped at word boundaries.

## `TextOverflow`

This enum describes the how the text appear if it is too wide to fit in the Text width.

### Values

* **`TextWrap.clip`**: The text will simpli be clipped.
* **`TextWrap.elide`**: The text will be ellided with `…`.

## `EventResult`

This enum describes whether an event was rejected or accepted by an event handler.

### Values

* **`EventResult.reject`**: The event is rejected by this event handler and may then be handled by parent item
* **`EventResult.accept`**: The event is accepted and won't be processed further

## `FillRule`

This enum describes the different ways of deciding what the inside of a shape described by a path shall be.

### Values

* **`FillRule.nonzero`**: The ["nonzero" fill rule as defined in SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule#nonzero).
* **`FillRule.evenodd`**: The ["evenodd" fill rule as defined in SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule#evenodd).
