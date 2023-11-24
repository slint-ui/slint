<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
## `TabWidget`

`TabWidget` is a container for a set of tabs. It can only have `Tab` elements as children and only one tab will be visible at
a time.

### Properties

-   **`current-index`** (_in_ _int_): The index of the currently visible tab

### Properties of the `Tab` element

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
