// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

slint::include_modules!();

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    let main_window = MainWindow::new().unwrap();

    virtual_keyboard::init(&main_window);

    let _ = main_window.show();
    let _ = main_window.run();
}

mod virtual_keyboard {
    use super::*;
    use slint::*;

    pub fn init(app: &MainWindow) {
        let weak = app.as_weak();

        app.window().on_virtual_keyboard_event({
            move |event| match event {
                VirtualKeyboardEvent::Show { .. } => {
                    weak.unwrap().global::<VirtualKeyboardHandler>().set_open(true);
                }
                VirtualKeyboardEvent::Hide => {
                    weak.unwrap().global::<VirtualKeyboardHandler>().set_open(false);
                }
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
