<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

## `TimePickerPopup`

Use the timer picker to select the time, in either 24-hour or 12-hour mode (AM/PM). 

### Properties

-   **`use-24-hour-format`**: (_in_ _bool_): If set to `true` 24 hours are displayed otherwise it is displayed in AM/PM mode. (default: system default, if cannot be determined then `true`) 
-   **`title`** (_in_ _string_): The text that is displayed at the top of the picker.
-   **`time`**: (_in_ struct _[`Time`](#struct-time)_): Set the initial displayed time.

### Callbacks

-   **`canceled()`**: The cancel button was clicked.
-   **`accepted(Time)`** The ok button was clicked.

### Example

```slint
import { TimePickerPopup, Button } from "std-widgets.slint";
export component Example inherits Window {
    width: 600px;
    height: 600px;

    time-picker-button := Button {
        text: @tr("Open TimePicker");

        clicked => {
            time-picker.show();
        }
    }

    time-picker := TimePickerPopup {
        canceled => {
            time-picker.close();
        }

        accepted(time) => {
            debug(time);
            time-picker.close();
        }
    }
}
```

### Struct `Time`

Defines a time with hours, minutes, and seconds.

#### Fields

-   **`hour`(int)**: The hour value (range from 0 to 23).
-   **`minute`(int)**: The minute value (range from 1 to 59).
-   **`second`(int)**: The second value (range form 1 to 59).
