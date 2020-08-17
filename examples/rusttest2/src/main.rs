/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
#![deny(unsafe_code)]

sixtyfps::include_modules!();

fn main() {
    let app = Hello::new();
    let app_weak = app.clone().as_weak();
    app.as_ref().on_plus_clicked(move || {
        let app = app_weak.upgrade().unwrap();
        app.as_ref().set_counter(app.as_ref().get_counter() + 1);
    });
    let app_weak = app.clone().as_weak();
    app.as_ref().on_minus_clicked(move || {
        let app = app_weak.upgrade().unwrap();
        app.as_ref().set_counter(app.as_ref().get_counter() - 1);
    });
    app.run();
}
