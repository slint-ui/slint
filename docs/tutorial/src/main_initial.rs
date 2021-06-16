/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
// ANCHOR: main
sixtyfps::sixtyfps! {
    MainWindow := Window {
        Text {
            text: "hello world";
            color: green;
        }
    }
}
fn main() {
    MainWindow::new().run();
}
// ANCHOR_END: main
