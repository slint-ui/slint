<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
## `ScrollView`

A Scrollview contains a viewport that is bigger than the view and can be
scrolled. It has scrollbar to interact with. The viewport-width and
viewport-height are calculated automatically to create a scollable view
except for when using a for loop to populate the elements. In that case
the viewport-width and viewport-height aren't calculated automatically
and must be set manually for scrolling to work. The ability to
automatically calculate the viewport-width and viewport-height when
using for loops may be added in the future and is tracked in issue #407.

### Properties

-   **`enabled`** (_in_ _bool_): Used to render the frame as disabled or enabled, but doesn't change behavior of the widget.
-   **`has-focus`** (_in-out_ _bool_): Used to render the frame as focused or unfocused, but doesn't change the behavior of the widget.
-   **`viewport-width`** and **`viewport-height`** (_in-out_ _length_): The `width` and `length` properties of the viewport
-   **`viewport-x`** and **`viewport-y`** (_in-out_ _length_): The `x` and `y` properties of the viewport. Usually these are negative
-   **`visible-width`** and **`visible-height`** (_out_ _length_): The size of the visible area of the ScrollView (not including the scrollbar)

### Example

```slint
import { ScrollView } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 200px;
    ScrollView {
        width: 200px;
        height: 200px;
        viewport-width: 300px;
        viewport-height: 300px;
        Rectangle { width: 30px; height: 30px; x: 275px; y: 50px; background: blue; }
        Rectangle { width: 30px; height: 30px; x: 175px; y: 130px; background: red; }
        Rectangle { width: 30px; height: 30px; x: 25px; y: 210px; background: yellow; }
        Rectangle { width: 30px; height: 30px; x: 98px; y: 55px; background: orange; }
    }
}
```
