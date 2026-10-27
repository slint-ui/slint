// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[allow(dead_code)]
// ANCHOR: main
fn main() {
    MainWindow::new().unwrap().run().unwrap();
}

slint::slint! {
    #[slint_edition = "1.19"]
    export component MainWindow inherits Window {
        Text {
            text: "hello world";
            color: green;
        }
    }
}
// ANCHOR_END: main
