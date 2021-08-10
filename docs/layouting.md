# Positioning and Layout of Elements

All visual elements are shown in a window. Their position is stored in the `x` and `y`
properties as coordinates relative to their parent element. The absolute position of an element
in a window is calculated by adding the parent's position to the element's position. If the
parent has a grandparent element, then that one is added as well. This calculation continues until
the top-level element is reached.

The size of visual elements is stored in the `width` and `height` properties.

You can create an entire graphical user interface by placing the elements in two different
ways:

* Explicitly - by setting the `x`, `y`, `width`, and `height` properties.
* Automatically - by using layout elements.

Explicit placement is great for static scenes with few elements. Layouts are suitable for
complex user interfaces, because the geometric relationship between the elements is
expressed in dedicated layout elements. This requires less effort to maintain and helps
to create scalable user interfaces.

## Explicit Placement

The following example places two rectangles into a window, a blue one and
a green one that is a child of the blue:

```60
// Explicit positioning
Example := Window {
    width: 200px;
    height: 200px;
    Rectangle {
        x: 100px;
        y: 70px;
        width: parent.width - x;
        height: parent.height - y;
        background: blue;
        Rectangle {
            x: 10px;
            y: 5px;
            width: 50px;
            height: 30px;
            background: green;
        }
    }
}
```

The position of both rectangles is fixed, as well as the size of the inner green one.
The outer blue rectangle however has a size that's automatically calculated using binding
expressions for the `width` and `height` properties. The calculation results in the
bottom left corner aligning with the corner of the window - it is updated whenever
the `width` and `height` of the window changes.

When specifying explicit values for any of the geometric properties, SixtyFPS requires
you to attach a unit to the number. You can choose between two different units:

* Logical pixels, using the `px` unit suffix. This is the recommended unit.
* Physical pixels, using the `phx` unit suffix

Logical pixels scale automatically with the device pixel ratio that your system is
configured with. For example, on a modern High-DPI display the device pixel ratio can be 2,
so every logical pixel occupies 2 physical pixels. On an older screen the user
interface scales without any adaptations.

Additionally, the `width` and `height` properties can also be specified as a `%` percentage
unit, which applies relative to the parent element. For example a `width: 50%` means half
of the parent's `width`.

The default values for `x` and `y` properties are 0, which means they align with their parent
on the screen.

The default values for `width` and `height` depend on the type of element. Some elements are sized
automatically based on their content, such as `Image`, `Text`, and most widgets. The following elements
do not have content and therefore default to fill their parent element:

* `Rectangle`
* `TouchArea`
* `FocusScope`
* `Flickable`
* `Clip`

## Automatic Placement using Layouts

SixtyFPS comes with different layout elements that automatically calculate the position and size of their children:

* `VerticalLayout` / `HorizontalLayout`: The children are placed along the vertical or horizontal axis.
* `GridLayout`: The children are placed in a grid of columns and rows.
* `PathLayout`: The children are placed along a path.

Layouts can also be nested, making it possible to create complex user interfaces.

You can tune the automatic placement using different constraints, to accommodate the design of your user
interface. For example each element has a minimum and a maximum size. Set these explicitly using the
following properties:

* `min-width`
* `min-height`
* `max-width`
* `max-height`

A layout element also affects the minimum and maximum size of its parent.

An element is considered to have a fixed size in a layout when the `width` and `height` is specified directly.

When there is extra space in a layout, elements can stretch along the layout axis. You can control this stretch
factor between the element and its siblings with these properties:

* `horizontal-stretch`
* `vertical-stretch`

A value of `0` means that the element will not be stretched at all; unless all siblings also have a stretch
factor of `0`. Then all the elements will be equally stretched.

## Common Properties on Layout Elements

All layout elements have the following properties in common:

* `spacing`: This controls the spacing between the children.
* `padding`: This specifies the padding within the layout, the space between the elements and the border of the
    layout.

For more fine grained control, the `padding` property can be split into properties for each side of the layout:

* `padding-left`
* `padding-right`
* `padding-top`
* `padding-bottom`

## `VerticalLayout` and `HorizontalLayout`

The `VerticalLayout` and `HorizontalLayout` elements place elements in a column or row.
By default, they will be stretched or shrunk so that they take the whole space, and their
alignment can be adjusted.

The following example places the blue and yellow rectangle in a row and evenly stretched
across the 200 logical pixels of `width`:

```60
// Stretch by default
Example := Window {
    width: 200px;
    height: 200px;
    HorizontalLayout {
        Rectangle { background: blue; min-width: 20px; }
        Rectangle { background: yellow; min-width: 30px; }
    }
}
```

The example below, on the other hand, specifies that the rectangles shell be aligned
to the start of the layout (the visual left). That results in no stretching but instead
the rectangles retain their specified minimum width:

```60
// Unless an alignment is specified
Example := Window {
    width: 200px;
    height: 200px;
    HorizontalLayout {
        alignment: start;
        Rectangle { background: blue; min-width: 20px; }
        Rectangle { background: yellow; min-width: 30px; }
    }
}
```

The example below nests two layouts for a more complex scene:

```60
Example := Window {
    width: 200px;
    height: 200px;
    HorizontalLayout {
        // Side panel
        Rectangle { background: green; width: 10px; }

        VerticalLayout {
            padding: 0px;
            //toolbar
            Rectangle { background: blue; height: 7px; }

            Rectangle {
                border-color: red; border-width: 2px;
                HorizontalLayout {
                    Rectangle { border-color: blue; border-width: 2px; }
                    Rectangle { border-color: green; border-width: 2px; }
                }
            }
            Rectangle {
                border-color: orange; border-width: 2px;
                HorizontalLayout {
                    Rectangle { border-color: black; border-width: 2px; }
                    Rectangle { border-color: pink; border-width: 2px; }
                }
            }
        }
    }
}
```

### Alignment

Each elements is sized according to their `width` or `height` is specified, otherwise it is
set to the minimum size which is set with the min-width or min-height property, or
the minimum size of an inner layout, whatever is bigger.
Then, the elements are placed according to the alignment.
The size of elements is bigger than the minimum size only if the alignment is stretch

This example show the different alignment possibilities

```60
Example := Window {
    width: 300px;
    height: 200px;
    VerticalLayout {
        HorizontalLayout {
            alignment: stretch;
            Text { text: "stretch (default)"; }
            Rectangle { background: blue; min-width: 20px; }
            Rectangle { background: yellow; min-width: 30px; }
        }
        HorizontalLayout {
            alignment: start;
            Text { text: "start"; }
            Rectangle { background: blue; min-width: 20px; }
            Rectangle { background: yellow; min-width: 30px; }
        }
        HorizontalLayout {
            alignment: end;
            Text { text: "end"; }
            Rectangle { background: blue; min-width: 20px; }
            Rectangle { background: yellow; min-width: 30px; }
        }
        HorizontalLayout {
            alignment: start;
            Text { text: "start"; }
            Rectangle { background: blue; min-width: 20px; }
            Rectangle { background: yellow; min-width: 30px; }
        }
        HorizontalLayout {
            alignment: center;
            Text { text: "center"; }
            Rectangle { background: blue; min-width: 20px; }
            Rectangle { background: yellow; min-width: 30px; }
        }
        HorizontalLayout {
            alignment: space-between;
            Text { text: "space-between"; }
            Rectangle { background: blue; min-width: 20px; }
            Rectangle { background: yellow; min-width: 30px; }
        }
        HorizontalLayout {
            alignment: space-around;
            Text { text: "space-around"; }
            Rectangle { background: blue; min-width: 20px; }
            Rectangle { background: yellow; min-width: 30px; }
        }
    }
}
```

### Stretch algorithm

When the `alignment` is set to stretch (the default), the elements are sized to their minimum size,
then the extra space is shared amongst element proportional to their stretch factor set with the
`horizontal-stretch` and `vertical-stretch` properties. But the size does not exceed the maximum size.
The stretch factor is a floating point number. The elements that have a default content size usually defaults to 0
while elements that default to the size of their parents defaults to 1.
An element of a stretch factor if 0 will keep its minimum size, unless all the other elements also have a stretch
factor of 0 or reached their maximum size.

Examples:

```60
Example := Window {
    width: 300px;
    height: 200px;
    VerticalLayout {
        // Same stretch factor (1 by default): the size is divided equally
        HorizontalLayout {
            Rectangle { background: blue; }
            Rectangle { background: yellow;}
            Rectangle { background: green;}
        }
        // Elements with a bigger min-width are given a bigger size before they expand
        HorizontalLayout {
            Rectangle { background: cyan; min-width: 100px;}
            Rectangle { background: magenta; min-width: 50px;}
            Rectangle { background: gold;}
        }
        // Stretch factor twice as big:  grows twice as much
        HorizontalLayout {
            Rectangle { background: navy; horizontal-stretch: 2;}
            Rectangle { background: gray; }
        }
        // All elements not having a maximum width have a stretch factor of 0 so they grow
        HorizontalLayout {
            Rectangle { background: red; max-width: 20px; }
            Rectangle { background: orange; horizontal-stretch: 0; }
            Rectangle { background: pink; horizontal-stretch: 0; }
        }
    }
}
```

### `for`

The VerticalLayout and Horizontal layout may also contain `for` or `if` expressions, and it does what one expect

```60
Example := Window {
    width: 200px;
    height: 50px;
    HorizontalLayout {
        Rectangle { background: green; }
        for t in [ "Hello", "World", "!" ] : Text {
            text: t;
        }
        Rectangle { background: blue; }
    }
}
```

## GridLayout

The GridLayout lays the element in a grid.
Each element gains the properties `row`, `col`, `rowspan`, and `colspan`.
One can either use a `Row` sub-element, or set the `row` property explicitly.
These properties must be statically known at compile time, so it is not possible to use arithmetic or depends on properties.
As of now, the use of `for` or `if` is not allowed in a grid layout.

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
        spacing: 0px;
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
