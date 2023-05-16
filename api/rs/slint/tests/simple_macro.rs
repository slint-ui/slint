// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use ::slint::slint;

#[test]
fn simple_window() {
    i_slint_backend_testing::init();
    slint!(export component X inherits Window{});
    X::new().unwrap();
}
#[test]
fn empty_stuff() {
    slint!();
    slint!(export struct Hei { abcd: bool });
    slint!(export global G { });
}
