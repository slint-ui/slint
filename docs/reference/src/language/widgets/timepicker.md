<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

## struct `Time`

Defines a time with hour minutes and seconds.

### Fields

-   **`hour`(int)**: The hour value (range from 0 to 23).
-   **`minute`(int)**: The minute value (range from 1 to 59).
-   **`second`(int)**: The second value (range form 1 to 59).

## `TimePicker`

A timer picker that is usd for selecting the time, in either 24-hour or AM/PM mode. 

### Properties

-   **`use-24-hour-format`**: (_in_ _bool_): If set to `true` 24 hours are displayed otherwise it is displayed in AM/PM mode. (default: system default, if cannot be determined then `true`) 
-   **`title`** (_in_ _string_): The text that is displayed at the top of the picker.
-   **`cancel-label`** (_in_ _string_): The text written in the cancel button.
-   **`ok-label`** (_in_ _string_): The text written in the ok button.
-   **`time`**: (_in_ _Time_): Set the initinal displayed time.

### Callbacks

-   **`cancled()`**: The cancel button was clicked.
-   **`accepted(Time)`** The ok button was clicked.

### Example

```slint
import { TimePicker, Button } from "std-widgets.slint";
export component Example inherits Window {
    width: 600px;
    height: 600px;

    time-picker-button := Button {
        text: @tr("Open TimePicker");

        clicked => {
            time-picker.show();
        }
    }

    time-picker := PopupWindow {
        width: 340px;
        height: 500px;
        close-on-click: false;

        TimePicker { 
            canceled => {
                time-picker.close();
            }

            accepted(time) => {
                debug(time);
                time-picker.close();
            }
        }
    }
}
```
