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

        app.global::<VirtualKeyboardHandler>().set_keys(default_keys());
        app.global::<VirtualKeyboardHandler>().set_secondary(false);

        app.window().on_virtual_keyboard_event({
            move |event| {
                match event {
                    VirtualKeyboardEvent::Show { .. } => {
                        weak.unwrap().global::<VirtualKeyboardHandler>().set_open(true);
                    }
                    VirtualKeyboardEvent::Hide => {
                        weak.unwrap().global::<VirtualKeyboardHandler>().set_open(false);
                    }
                }

                false
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

        app.global::<VirtualKeyboardHandler>().on_switch_keyboard(move || {
            let secondary = !weak.unwrap().global::<VirtualKeyboardHandler>().get_secondary();
            weak.unwrap().global::<VirtualKeyboardHandler>().set_secondary(secondary);

            if !secondary {
                weak.unwrap().global::<VirtualKeyboardHandler>().set_keys(default_keys());
            } else {
                weak.unwrap().global::<VirtualKeyboardHandler>().set_keys(secondary_keys());
            }
        });
    }

    fn default_keys() -> ModelRc<ModelRc<KeyModel>> {
        VecModel::from_slice(&[
            VecModel::from_slice(&[
                KeyModel { key: "q".into(), shift_key: "Q".into() },
                KeyModel { key: "w".into(), shift_key: "W".into() },
                KeyModel { key: "e".into(), shift_key: "E".into() },
                KeyModel { key: "r".into(), shift_key: "R".into() },
                KeyModel { key: "t".into(), shift_key: "T".into() },
                KeyModel { key: "y".into(), shift_key: "Y".into() },
                KeyModel { key: "u".into(), shift_key: "U".into() },
                KeyModel { key: "i".into(), shift_key: "I".into() },
                KeyModel { key: "o".into(), shift_key: "O".into() },
                KeyModel { key: "p".into(), shift_key: "P".into() },
            ]),
            VecModel::from_slice(&[
                KeyModel { key: "q".into(), shift_key: "Q".into() },
                KeyModel { key: "w".into(), shift_key: "W".into() },
                KeyModel { key: "e".into(), shift_key: "E".into() },
                KeyModel { key: "r".into(), shift_key: "R".into() },
                KeyModel { key: "t".into(), shift_key: "T".into() },
                KeyModel { key: "y".into(), shift_key: "Y".into() },
                KeyModel { key: "u".into(), shift_key: "U".into() },
                KeyModel { key: "i".into(), shift_key: "I".into() },
                KeyModel { key: "o".into(), shift_key: "O".into() },
                KeyModel { key: "p".into(), shift_key: "P".into() },
            ]),
            VecModel::from_slice(&[
                KeyModel { key: "q".into(), shift_key: "Q".into() },
                KeyModel { key: "w".into(), shift_key: "W".into() },
                KeyModel { key: "e".into(), shift_key: "E".into() },
                KeyModel { key: "r".into(), shift_key: "R".into() },
                KeyModel { key: "t".into(), shift_key: "T".into() },
                KeyModel { key: "y".into(), shift_key: "Y".into() },
                KeyModel { key: "u".into(), shift_key: "U".into() },
                KeyModel { key: "i".into(), shift_key: "I".into() },
                KeyModel { key: "o".into(), shift_key: "O".into() },
                KeyModel { key: "p".into(), shift_key: "P".into() },
            ]),
        ])
    }

    fn secondary_keys() -> ModelRc<ModelRc<KeyModel>> {
        VecModel::from_slice(&[
            VecModel::from_slice(&[
                KeyModel { key: "1".into(), shift_key: "[".into() },
                KeyModel { key: "2".into(), shift_key: "]".into() },
                KeyModel { key: "3".into(), shift_key: "{".into() },
                KeyModel { key: "4".into(), shift_key: "}".into() },
                KeyModel { key: "5".into(), shift_key: "#".into() },
                KeyModel { key: "6".into(), shift_key: "%".into() },
                KeyModel { key: "7".into(), shift_key: "^".into() },
                KeyModel { key: "8".into(), shift_key: "*".into() },
                KeyModel { key: "9".into(), shift_key: "+".into() },
                KeyModel { key: "0".into(), shift_key: "=".into() },
            ]),
            VecModel::from_slice(&[
                KeyModel { key: "-".into(), shift_key: "_".into() },
                KeyModel { key: "/".into(), shift_key: "\\".into() },
                KeyModel { key: ":".into(), shift_key: "|".into() },
                KeyModel { key: ";".into(), shift_key: "~".into() },
                KeyModel { key: "(".into(), shift_key: "<".into() },
                KeyModel { key: ")".into(), shift_key: ">".into() },
                KeyModel { key: "€".into(), shift_key: "$".into() },
                KeyModel { key: "&".into(), shift_key: "€".into() },
                KeyModel { key: "@".into(), shift_key: "°".into() },
                KeyModel { key: "'".into(), shift_key: "#".into() },
            ]),
            VecModel::from_slice(&[
                KeyModel { key: ".".into(), shift_key: ".".into() },
                KeyModel { key: ",".into(), shift_key: ",".into() },
                KeyModel { key: "?".into(), shift_key: "?".into() },
                KeyModel { key: "!".into(), shift_key: "!".into() },
                KeyModel { key: "'".into(), shift_key: "'".into() },
            ]),
        ])
    }
}
