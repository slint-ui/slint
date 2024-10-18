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
    This property can only be set in a binding and cannot be accessed in an expression.

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