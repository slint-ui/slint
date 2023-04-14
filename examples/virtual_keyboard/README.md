# Virtual Keyboard Example

This example application demonstrates how to implement and display a custom virtual keyboard in Slint.
It has three different building blocks:

1. The virtual keyboard itself: This is implemented in `virtual_keyboard.slint` as a re-usable component.
   The application is responsible for placing it in the scene, typically as the last item in the root component.
2. Keyboard visibility: When a `TextInput` element receives the focus, either by the user clicking on it or programmatically
   via a call to `focus()`, it sets the global `TextInputInterface.text-input-focused` property to true. Similarly,
   when the focus is lost, this property is set to false again. Use this property to control visibility of the virtual keyboard.
3. Interaction: When the user clicks on a key in the virtual keyboard, the application needs to simulate a key event as if the user
   pressed the key on a real keyboard. The virtual keyboard invokes `VirtualKeyboardHandler`'s `key_pressed` callback. You need
   to set this callback to dispatch a key event to the `slint::Window`. Slint takes care of routing it to the currently focused
   `TextInput`. In Rust, call `slint::Window::dispatch_event(slint::platform::WindowEvent::KeyPressed{...})` to dispatch
   the event; in C++ call `slint::Window::dispatch_key_press_event(...)`. Subsequently, the you should dispatch a key
   release event using the same family of functions.

## Example

```slint
import { VirtualKeyboard } from "virtual_keyboard.slint"

export MainWindow inherits Window {
    HorizontalLayout {
        TextInput {}
    }

    VirtualKeyboard {
        visible: TextInputInterface.text-input-focused;
    }
}
```

### Rust Application Code

```rust
fn main() {
    let app = App::new().unwrap();
    
    let weak = app.as_weak();
    app.global::<VirtualKeyboardHandler>().on_key_pressed({
        let weak = weak.clone();
        move |key| {
            weak.unwrap()
                .window()
                .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key.clone() });
            weak.unwrap()
                .window()
                .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: key });
        }
    });

    app.run();
}
```

### C++ Application Code

```cpp
int main()
{
    auto main_window = MainWindow::create();
    app->global<VirtualKeyboardHandler>().on_key_pressed([=](auto key) {
        app->window().dispatch_key_press_event(key);
        app->window().dispatch_key_release_event(key);
    });
    main_window->run();
}

```
