# Focus Handling

Certain elements such as ```TextInput``` accept not only input from the mouse/finger but
also key events originating from (virtual) keyboards. In order for an item to receive
these events, it must have the focus. This is visible through the `has-focus` property.

You can manually activate the focus on an element by calling `focus()`:

```slint
import { Button } from "std-widgets.slint";

export component App inherits Window {
    VerticalLayout {
        alignment: start;
        Button {
            text: "press me";
            clicked => { input.focus(); }
        }
        input := TextInput {
            text: "I am a text input field";
        }
    }
}
```

If you have wrapped the `TextInput` in a component, then you can forward such a focus activation
using the `forward-focus` property to refer to the element that should receive it:

```slint
import { Button } from "std-widgets.slint";

component LabeledInput inherits GridLayout {
    forward-focus: input;
    Row {
        Text {
            text: "Input Label:";
        }
        input := TextInput {}
    }
}

export component App inherits Window {
    GridLayout {
        Button {
            text: "press me";
            clicked => { label.focus(); }
        }
        label := LabeledInput {
        }
    }
}
```

If you use the `forward-focus` property on a `Window`, then the specified element will receive
the focus the very first time the window receives the focus - it becomes the initial focus element.
