// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#[allow(dead_code)]
// ANCHOR: main
fn main() {
    MainWindow::new().run();
}

slint::slint! {
    MainWindow := Window {
        Text {
            text: "hello world";
            color: green;
        }
    }
}
// ANCHOR_END: main
