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
