<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

## `DatePickerPopup`

Use a date picker to let the user select a date.

### Properties

-   **`title`** (_in_ _string_): The text that is displayed at the top of the picker.
-   **`date`**: (_in_ _struct [`Date`](#struct-date)_): Set the initial displayed date.

### Callbacks

-   **`canceled()`**: The cancel button was clicked.
-   **`accepted(Date)`** The ok button was clicked.

### Example

```slint
import { DatePickerPopup, Button } from "std-widgets.slint";
export component Example inherits Window {
    width: 600px;
    height: 600px;

    date-picker-button := Button {
        text: @tr("Open Date Picker");

        clicked => {
            date-picker.show();
        }
    }

    date-picker := DatePickerPopup {
        width: 360px;
        height: 524px;
        close-on-click: false;

        accepted(date) => {
            date-picker.close();
        }
    }
}
```

### Struct `Date`

Defines a date with day, month, and year.

#### Fields

-   **`day`(int)**: The day value (range from 1 to 31).
-   **`month`(int)**: The month value (range from 1 to 12).
-   **`year`(int)**: The year value.
