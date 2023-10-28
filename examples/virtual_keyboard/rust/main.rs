// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

pub fn main() {
    let main_window = MainWindow::new().unwrap();

    virtual_keyboard::init(&main_window);

    main_window.run().unwrap();
}

mod virtual_keyboard {
    use super::*;
    use slint::*;
    use slint::private_unstable_api::re_exports::TextInputInterface;

    pub fn init(app: &MainWindow) {
        let weak = app.as_weak();
        app.global::<VirtualKeyboardHandler>().on_key_pressed({
            move |key| {
                weak.unwrap()
                    .window()
                    .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key.clone() });
                weak.unwrap()
                    .window()
                    .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: key });
            }
        });

        // An example of the text input focus callback.
        // Can be used to show or hide the virtual keyboard using system's APIs.
        app.global::<TextInputInterface>().on_text_input_focus_changed(|change| {
            println!("text-input-focus-changed: {:?}", change);
        });
    }
}
