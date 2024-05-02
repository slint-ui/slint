<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

## struct `Time`

Defines a time with hour minutes and seconds.

### Fields

-   **`hour`(int)**: The hour value.
-   **`minute`(int)**: The minute value.
-   **`second`(int)**: The second value.
## `TimePicker`

A timer picker that is usd for selecting the time, in either 24-hour or AM/PM mode. 

### Properties

-   **`title`** (_in_ _string_): The text that is displayed at the top of the picker.
-   **`ok-label`** (_in_ _string_): The text written in the ok button.
-   **`cancel-label`** (_in_ _string_): The text written in the cancel button.
-   **`is-twenty-four-hour`**: (_in_ _bool_): Sets to true to enable 24 hour selection otherwise it is displayed in AM/PM mode.  
-   **`curren-time`**: (_in-out_ _Time_): Gets and sets the current selected time.. 

### Callbacks

-   **`cancled()`**: The cancel button was clicked.
-   **`accepted(Time)`** The ok button was clicked.

### Example

```slint
import { TimePicker } from "std-widgets.slint";
export component Example inherits Window {
  width: 200px;
  height: 130px;
  
  time-picker-popup := Popup {
    TimePicker {
      
    }
  }

  Button {
    text: "Open time picker";
    clicked => {
      time-picker-popup.show();
    }
  }
}
```
