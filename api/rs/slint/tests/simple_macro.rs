// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

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
    slint!(global G := { });
}
