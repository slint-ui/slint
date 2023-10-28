<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Builtin Global Singletons

## `TextInputInterface`

The `TextInputInterface.text-input-focused` property can be used to find out if a `TextInput` element has the focus.
If you're implementing your own virtual keyboard, this property is an indicator whether the virtual keyboard should be shown or hidden.

### Properties

-   **`text-input-focused`** (_bool_): True if an `TextInput` element has the focus; false otherwise. 

### Callbacks (only available in Rust)

-   **`text-input-focus-changed`** (_change_): a callback that gets invoked every time a text input is focused or unfocused.
    
    Example:
    ```rust
    window.global::<TextInputInterface>.on_text_input_focus_changed(|change| match change {
        TextInputFocusChangeEvent::Focused => system::show_popup_keyboard(),
        TextInputFocusChangeEvent::Unfocused => system::hide_popup_keyboard(),
    });
    ```

### Example

```slint
import { LineEdit } from "std-widgets.slint";

component VKB {
    Rectangle { background: yellow; }
}

export component Example inherits Window {
    width: 200px;
    height: 100px;
    VerticalLayout {
        LineEdit {}
        FocusScope {}
        if TextInputInterface.text-input-focused: VKB {}
    }
}
```
