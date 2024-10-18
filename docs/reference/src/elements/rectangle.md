## `Rectangle`

By default, a `Rectangle` is just an empty item that shows nothing. By setting a color or configuring a border,
it's then possible to draw a rectangle on the screen.

When not part of a layout, its width and height default to 100% of the parent element.

### Properties

-   **`background`** (_in_ _brush_): The background brush of this `Rectangle`, typically a color. (default value: `transparent`)
-   **`border-color`** (_in_ _brush_): The color of the border. (default value: `transparent`)
-   **`border-radius`** (_in_ _length_): The size of the radius. (default value: 0)
-   **`border-top-left-radius`**, **`border-top-right-radius`**, **`border-bottom-left-radius`** and **`border-bottom-right-radius`** (_in_ _length_): Set these properties to override the radius for specific corners.
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