// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use ::slint::slint;

#[test]
fn simple_window() {
    slint!(export component X inherits Window{});
    X::new().unwrap();
}
#[test]
fn empty_stuff() {
    slint!();
    slint!(export struct Hei { abcd: bool });
    slint!(export global G { });
}
