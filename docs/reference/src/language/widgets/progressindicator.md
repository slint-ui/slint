<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
## `ProgressIndicator`

The `ProgressIndicator` informs the user about the status of an on-going operation, such as loading data from the network.

### Properties

-   **`indeterminate`**: (_in_ _bool_): Set to true if the progress of the operation cannot be determined by value (default value: `false`).
-   **`progress`** (_in_ _float_): Percentage of completion, as value between 0 and 1. Values less than 0 or greater than 1 are capped.

### Example

```slint
import { ProgressIndicator } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    ProgressIndicator {
        width: parent.width;
        height: parent.height;
        progress: 50%;
    }
}
```
