<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# States

The `states` statement allows to declare states and set properties of multiple elements in one go:

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;
    default-font-size: 24px;

    label := Text { }
    ta := TouchArea {
        clicked => {
            active = !active;
        }
    }
    property <bool> active: true;
    states [
        active when active && !ta.has-hover: {
            label.text: "Active";
            root.background: blue;
        }
        active-hover when active && ta.has-hover: {
            label.text: "Active\nHover";
            root.background: green;
        }
        inactive when !active: {
            label.text: "Inactive";
            root.background: gray;
        }
    ]
}
```

In this example, the `active` and `active-hovered` states are defined depending on the value of the `active`
boolean property and the `TouchArea`'s `has-hover`. When the user hovers the example with the mouse, it will toggle between a blue and a green background,
and adjust the text label accordingly. Clicking toggles the `active` property and thus enters the `inactive` state.

## Transitions

Transitions bind animations to state changes.

This example defines two transitions. First the `out` keyword is used to animate
all properties for 800ms when leaving the `disabled` state. The second
transition uses the `in` keyword to animate the background when transitioning
into the `down` state.

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;

    text := Text { text: "hello"; }
    in-out property<bool> pressed;
    in-out property<bool> is-enabled;

    states [
        disabled when !root.is-enabled : {
            background: gray; // same as root.background: gray;
            text.color: white;
            out {
                animate * { duration: 800ms; }
            }
        }
        down when pressed : {
            background: blue;
            in {
                animate background { duration: 300ms; }
            }
        }
    ]
}
```
