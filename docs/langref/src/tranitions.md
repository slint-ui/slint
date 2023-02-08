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
