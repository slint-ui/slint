## `Timer`
<!-- FIXME: Timer is not really an element so it doesn't really belong in the `Builtin Elements` section. -->

Use the Timer pseudo-element to schedule a callback at a given interval.
The timer is only running when the `running` property is set to `true`. To stop or start the timer, set that property to `true` or `false`.
It can be also set to a binding expression.
When already running, the timer will be restarted if the `interval` property is changed.

:::{note}
The default value for `running` is `true`, so if you don't specify it, it will be running.
:::

:::{note}
Timer is not an actual element visible in the tree, therefore it doesn't have the common properties such as `x`, `y`, `width`, `height`, etc. It also doesn't take room in a layout and cannot have any children or be inherited from.
:::

### Properties

 -  **`interval`** (_in_ _duration_): The interval between timer ticks. This property is mandatory.
 -  **`running`** (_in_ _bool_): `true` if the timer is running. (default value: `true`)

### Callbacks

 -  **`triggered()`**: Invoked every time the timer ticks (every `interval`).

### Example

This example shows a timer that counts down from 10 to 0 every second:

```slint
import { Button } from "std-widgets.slint";
export component Example inherits Window {
    property <int> value: 10;
    timer := Timer {
        interval: 1s;
        running: true;
        triggered() => {
            value -= 1;
            if (value == 0) {
                self.running = false;
            }
        }
    }
    HorizontalLayout {
        Text { text: value; }
        Button {
            text: "Reset";
            clicked() => { value = 10; timer.running = true; }
        }
    }
}
```

