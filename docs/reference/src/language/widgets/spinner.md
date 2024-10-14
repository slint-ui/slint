<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

## `Spinner`

The `Spinner` informs the user about the status of an on-going operation, such as loading data from the network. It provides the same properties as
[`ProgressIndicator`](./progressindicator.md) but differs in shape.

### Properties

-   **`indeterminate`**: (_in_ _bool_): Set to true if the progress of the operation cannot be determined by value (default value: `false`).
-   **`progress`** (_in_ _float_): Percentage of completion, as value between 0 and 1. Values less than 0 or greater than 1 are capped.

### Example

```slint
import { Spinner } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    Spinner {
        progress: 50%;
    }
}
```
