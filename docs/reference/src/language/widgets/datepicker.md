<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

## struct `Date`

Defines a date with day, month, and year

### Fields

-   **`day`(int)**: The day value (range from 1 to 31).
-   **`month`(int)**: The month value (range from 1 to 12).
-   **`year`(int)**: The year value.

## `DatePicker`

Use a date picker to let the user select a date.

### Properties

-   **`title`** (_in_ _string_): The text that is displayed at the top of the picker.
-   **`cancel-label`** (_in_ _string_): The text written in the cancel button.
-   **`ok-label`** (_in_ _string_): The text written in the ok button.
-   **`date`**: (_in_ _DAte_): Set the initinal displayed date. 

### Callbacks

-   **`canceled()`**: The cancel button was clicked.
-   **`accepted(Date)`** The ok button was clicked.

### Example

```slint
import { DatePicker, Button } from "std-widgets.slint";
export component Example inherits Window {
    width: 600px;
    height: 600px;

    date-picker-button := Button {
        text: @tr("Open Date Picker");

        clicked => {
            date-picker.show();
        }
    }

    date-picker := PopupWindow {
        width: 360px;
        height: 524px;
        close-on-click: false;

        DatePicker {
            canceled => {
                date-picker.close();
            }

            accepted(date) => {
                debug(date);
                date-picker.close();
            }
        }
    }
}
```
