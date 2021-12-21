// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use ::sixtyfps::sixtyfps;

#[test]
fn simple_window() {
    sixtyfps!(X := Window{});
    X::new();
}
#[test]
fn empty_stuff() {
    sixtyfps!();
    sixtyfps!(struct Hei := { abcd: bool });
}
