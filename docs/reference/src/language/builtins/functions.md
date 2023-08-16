<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Builtin Functions

## `animation-tick() -> duration`

This function returns a monotonically increasing time, which can be used for animations.
Calling this function from a binding will constantly re-evaluate the binding.
It can be used like so: `x: 1000px + sin(animation-tick() / 1s * 360deg) * 100px;` or `y: 20px * mod(animation-tick(), 2s) / 2s `

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;

    Rectangle {
        y:0;
        background: red;
        height: 50px;
        width: parent.width * mod(animation-tick(), 2s) / 2s;
    }

    Rectangle {
        background: blue;
        height: 50px;
        y: 50px;
        width: parent.width * abs(sin(360deg * animation-tick() / 3s));
    }
}
```

## `debug(...)`

The debug function can take one or multiple values as arguments, prints them, and returns nothing.
