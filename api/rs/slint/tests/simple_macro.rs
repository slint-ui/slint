// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use ::slint::slint;

#[test]
fn simple_window() {
    slint!(X := Window{});
    X::new();
}
#[test]
fn empty_stuff() {
    slint!();
    slint!(struct Hei := { abcd: bool });
}
