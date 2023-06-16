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

    pub fn init(app: &MainWindow) {
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
    }
}
