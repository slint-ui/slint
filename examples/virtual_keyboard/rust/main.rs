// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

slint::include_modules!();

pub fn main() {
    let main_window = MainWindow::new().unwrap();

    virtual_keyboard::init(&main_window);

    main_window.run().unwrap();
}

mod virtual_keyboard {
    use super::*;
    use slint::{platform::InputMethodRequestResult, *};

    pub fn init(app: &MainWindow) {
        let weak = app.as_weak();

        app.window().on_input_method_request({
            move |event| {
                match event {
                    InputMethodRequest::Activate { .. } => {
                        weak.unwrap().global::<VirtualKeyboardHandler>().set_open(true);
                    }
                    InputMethodRequest::Deactivate => {
                        weak.unwrap().global::<VirtualKeyboardHandler>().set_open(false);
                    }
                    _ => unreachable!(),
                }

                InputMethodRequestResult::PreventDefault
            }
        });

        let weak = app.as_weak();
        app.global::<VirtualKeyboardHandler>().on_key_pressed({
            let weak = weak.clone();
            move |key| {
                weak.unwrap()
                    .window()
                    .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key });
            }
        });
    }
}
