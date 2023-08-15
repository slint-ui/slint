## `TabWidget`

`TabWidget` is a container for a set of tabs. It can only have `Tab` elements as children and only one tab will be visible at
a time.

### Properties

-   **`content-min-width`** and **`content-min-height`** (_out_ _length_): The minimum width and height of the contents
-   **`content-width`** and **`content-height`** (_out_ _length_): The width and height of the contents
-   **`content-x`** and **`content-y`** (_out_ _length_): The x and y position of the contents
-   **`current-focused`** (_in_ _int_): The index of the tab that has focus. This tab may or may not be visible.
-   **`current-index`** (_in_ _int_): The index of the currently visible tab
-   **`tabbar-preferred-width`** and **`tabbar-preferred-height`** (_in_ _length_): The preferred width and height of the tab bar
-   **`tabbar-width`** and **`tabbar-height`** (_out_ _length_): The width and height of the tab bar
-   **`tabbar-x`** and **`tabbar-y`** (_out_ _length_): The x and y position of the tab bar

### Properties of the `Tab` element

-   **`current-focused`** (_out_ _int_): The index of this tab that has focus at this time or -1 if none is focused
-   **`enabled`**: (_in_ _bool_): Defaults to true. When false, the tab can't be activated
-   **`icon`** (_in_ _image_): The image on the tab
-   **`num-tabs`** (_out_ _int_): The number of tabs in the current `TabBar`
-   **`tab-index`** (_out_ _int_): The index of this tab
-   **`title`** (_in_ _string_): The text written on the tab

### Example

```slint
import { TabWidget } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 200px;
    TabWidget {
        Tab {
            title: "First";
            Rectangle { background: orange; }
        }
        Tab {
            title: "Second";
            Rectangle { background: pink; }
        }
    }
}
```
