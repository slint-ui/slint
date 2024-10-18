## `VerticalLayout` and `HorizontalLayout`

These layouts place their children next to each other vertically or horizontally.
The size of elements can either be fixed with the `width` or `height` property, or if they aren't set
they will be computed by the layout respecting the minimum and maximum sizes and the stretch factor.

### Properties

-   **`spacing`** (_in_ _length_): The distance between the elements in the layout.
-   **`padding`** (_in_ _length_): the padding within the layout.
-   **`padding-left`**, **`padding-right`**, **`padding-top`** and **`padding-bottom`** (_in_ _length_): Set these properties to override the padding on specific sides.
-   **`alignment`** (_in_ _enum [`LayoutAlignment`](../language/builtins/enums.md#layoutalignment)_): Set the alignment. Matches the CSS flex box.

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
