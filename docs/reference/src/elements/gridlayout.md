## `GridLayout`

`GridLayout` places elements on a grid.

Cell elements inside a `GridLayout` obtain the following new properties. Any bindings to these properties must be compile-time constants:

-   **`row`** (_in_ _int_): The index of the element's row within the grid. Setting this property resets the element's column to zero, unless explicitly set.
-   **`col`** (_in_ _int_): The index of the element's column within the grid. Set this property to override the sequential column assignment (e.g., to skip a column).
-   **`rowspan`** (_in_ _int_): The number of rows this element should span. (default value: `1`)
-   **`colspan`** (_in_ _int_): The number of columns this element should span. (default value: `1`)

To implicitly sequentially assign row indices&mdash;just like with `col`&mdash;wrap cell elements in `Row` elements.

The following example creates a 2-by-2 grid with `Row` elements, omitting one cell:

```slint
import { Button } from "std-widgets.slint";
export component Foo inherits Window {
    width: 200px;
    height: 100px;
    GridLayout {
        Row { // children implicitly on row 0
            Button { col: 1; text: "Top Right"; } // implicit column after this would be 2
        }
        Row { // children implicitly on row 1
            Button { text: "Bottom Left"; }  // implicitly in column 0...
            Button { text: "Bottom Right"; } // ...and 1
        }
    }
}
```

The following example creates the same grid using the `row` property. Row indices must be taken care of manually:

```slint
import { Button } from "std-widgets.slint";
export component Foo inherits Window {
    width: 200px;
    height: 100px;
    GridLayout {
        Button { row: 0; col: 1; text: "Top Right"; } // `row: 0;` could even be left out at the start
        Button { row: 1; text: "Bottom Left"; } // new row, implicitly resets column to 0
        Button { text: "Bottom Right"; } // same row, sequentially assigned column 1
    }
}
```

`GridLayout` covers its entire surface with cells. Cells are not aligned. The elements constituting the cells will be stretched inside their allocated space, unless their size constraints&mdash;like, e.g., `min-height` or `max-width`&mdash;work against this.

### Properties

-   **`spacing`** (_in_ _length_): The distance between the elements in the layout.
-   **`spacing-horizontal`**, **`spacing-vertical`** (_in_ _length_):
    Set these properties to override the spacing on specific axes.
-   **`padding`** (_in_ _length_): The padding around the grid structure as a whole.
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