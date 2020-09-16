/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

sixtyfps::include_modules!();

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    type TodoModelData = (bool, sixtyfps::SharedString);
    let todo_model = Rc::new(sixtyfps::VecModel::<TodoModelData>::from(vec![
        (true, "Implement the .60 file".into()),
        (true, "Do the rust part".into()),
        (false, "Make the C++ code".into()),
        (false, "???".into()),
        (false, "Profit".into()),
    ]));

    let main_window = MainWindow::new();
    main_window.as_ref().on_todo_added({
        let todo_model = todo_model.clone();
        move |text| todo_model.push((true, text))
    });

    main_window.set_todo_model(Some(todo_model));

    main_window.run();
}
