# Element positioning and layouting

Most elements have `x`, `y`, `width` and `height` properties which specify their geometry.
There are two ways to position an element on the screen: either by setting these properties explicitly, or by using a layout.


## Explicit positioning


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
        color: blue;
        Rectangle {
            x: 10px;
            y: 5px;
            width: 50px;
            height: 30px;
            color: green;
        }
    }
}
```

The `x` and `y` properties are relative to the parent. They can be specified in

*  `px`: logical pixels, scaled with the device pixel ratio
*  `phx`: physical pixels

The default value for `x` and `y` is always 0.

The `width` and `height` properties are also specified in pixels. Additionaly, they can also take a value in `%`, in that case, this is the ratio compared to the parent element.
The default values for `width` and `height` depends on the element.
Some elements have content and had their size is based on their content. This is the case for `Image` or `Text` and most widgets.
Elements that do not have content, default to fill the parent element. For example: `Rectangle`, `TouchArea`, `FocusScope`, `Flickable`,
and `Clip`.

## Layouts

There are different kinds of layouts, but they all share some common traits.
Layouts are responsible for positioning their direct sub-elements.
Each element has a minimum and maximum size which can be set with the `minimum_width`, `minimum_height`, `maximum_width`, and  `maximum_height` properties.
When the `width` or `height` is specified directly, it is considered to be of fixed size.
If an element contains a layout, it also impacts the minimum and maximum size of that element.
The `horizontal_stretch` and `vertical_stretch` properties specify how much an element stretches proportionally to other elements.

Layouts have a `spacing` and `padding` property. Their default value is defined by the widget style.
`padding` can be split in `padding-left`, `padding-right`, `padding-bottom` and `padding-top`.

## VerticalLayout and HorizontalLayout

These layout the widgets in a column (HorizontalLayout) or in a row (VerticalLayout).
By default, the elements will be stretched or shrinked so that they take the whole space, but this can be adjusted with the alignment.

```60
// Stretch by default
Example := Window {
    width: 200px;
    height: 200px;
    HorizontalLayout {
        Rectangle { color: blue; minimum_width: 20px; }
        Rectangle { color: yellow; minimum_width: 30px; }
    }
}
```

```60
// Unless an alignment is specified
Example := Window {
    width: 200px;
    height: 200px;
    HorizontalLayout {
        alignment: start;
        Rectangle { color: blue; minimum_width: 20px; }
        Rectangle { color: yellow; minimum_width: 30px; }
    }
}
```

It can be convinient to put layout within another to make complex UI


```60
Example := Window {
    width: 200px;
    height: 200px;
    HorizontalLayout {
        // Side panel
        Rectangle { color: green; width: 10px; }

        VerticalLayout {
            padding: 0px;
            //toolbar
            Rectangle { color: blue; height: 7px; }

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
set to the minimum size which is set with the minimum-width or minimum-height property, or
the minimum size of an inner layout, whateer is bigger.
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
            Rectangle { color: blue; minimum_width: 20px; }
            Rectangle { color: yellow; minimum_width: 30px; }
        }
        HorizontalLayout {
            alignment: start;
            Text { text: "start"; }
            Rectangle { color: blue; minimum_width: 20px; }
            Rectangle { color: yellow; minimum_width: 30px; }
        }
        HorizontalLayout {
            alignment: end;
            Text { text: "end"; }
            Rectangle { color: blue; minimum_width: 20px; }
            Rectangle { color: yellow; minimum_width: 30px; }
        }
        HorizontalLayout {
            alignment: start;
            Text { text: "start"; }
            Rectangle { color: blue; minimum_width: 20px; }
            Rectangle { color: yellow; minimum_width: 30px; }
        }
        HorizontalLayout {
            alignment: center;
            Text { text: "center"; }
            Rectangle { color: blue; minimum_width: 20px; }
            Rectangle { color: yellow; minimum_width: 30px; }
        }
        HorizontalLayout {
            alignment: space-between;
            Text { text: "space-between"; }
            Rectangle { color: blue; minimum_width: 20px; }
            Rectangle { color: yellow; minimum_width: 30px; }
        }
        HorizontalLayout {
            alignment: space-around;
            Text { text: "space-around"; }
            Rectangle { color: blue; minimum_width: 20px; }
            Rectangle { color: yellow; minimum_width: 30px; }
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
        // Same stretch factor (1 by default): the size is devided equally
        HorizontalLayout {
            Rectangle { color: blue; }
            Rectangle { color: yellow;}
            Rectangle { color: green;}
        }
        // Elements with a bigger minimum-width are given a bigger size before they expand
        HorizontalLayout {
            Rectangle { color: cyan; minimum-width: 100px;}
            Rectangle { color: magenta; minimum-width: 50px;}
            Rectangle { color: gold;}
        }
        // Stretch factor twice as big:  grows twice as much
        HorizontalLayout {
            Rectangle { color: navy; horizontal-stretch: 2;}
            Rectangle { color: gray; }
        }
        // All elements not having a maximum width have a stretch factor of 0 so they grow
        HorizontalLayout {
            Rectangle { color: red; maximum-width: 20px; }
            Rectangle { color: orange; horizontal-stretch: 0; }
            Rectangle { color: pink; horizontal-stretch: 0; }
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
        Rectangle { color: green; }
        for t in [ "Hello", "World", "!" ] : Text {
            text: t;
        }
        Rectangle { color: blue; }
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
            Rectangle { color: red; }
            Rectangle { color: blue; }
        }
        Row {
            Rectangle { color: yellow; }
            Rectangle { color: green; }
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
        Rectangle { color: red; }
        Rectangle { color: blue; }
        Rectangle { color: yellow; row: 1; }
        Rectangle { color: green; }
        Rectangle { color: black; col: 2; row: 0; }
    }
}
```

## `PathLayout`

FIXME: write docs