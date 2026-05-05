// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[satchel::test]
fn text_input_password() {
    slint::slint! {
        import { CheckBox } from "std-widgets.slint";
        export component MainWindow inherits Window {
            width: 600px;
            height: 600px;

            out property text-preferred-height <=> text-input.preferred-height;
            in-out property <string> text <=> text-input.text;

           VerticalLayout {
            text-input:= TextInput {
                input-type: InputType.password;
                height: self.preferred-height;
            }

            Rectangle {
                background: red;
            }
           }
        }
    }

    let app = MainWindow::new().unwrap();
    assert_eq!(app.get_text(), "");
    let preferred_height_empty = app.get_text_preferred_height();

    app.set_text("hello".into());

    assert_eq!(
        app.get_text_preferred_height(),
        preferred_height_empty,
        "The preferred height must not change if there is a character entered or not"
    );
    // let _ = app.run(); // Just for testing
}
