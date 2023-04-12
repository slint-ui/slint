# Virtual Keyboard Example

This example application demonstrates how to implement and display a custom virtual keyboard in Slint.

To check if the virtual keyboard should be open e.g. if a `TextInput` gets focus the property `TextInputInterface.text-input-focused` can be used.

## Example

```slint
import { MyVirtualKeyboard } from "virtual_keyboard.slint"

export MainWindow inherits Window {
    HorizontalLayout {
        TextInput {}
    }

    MyVirtualKeyboard {
        visible: TextInputInterface.text-input-focused;
    }
}
```