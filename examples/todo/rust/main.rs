// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use slint::{FilterModel, Model, SortModel};
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let todo_model = Rc::new(slint::VecModel::<TodoItem>::from(vec![
        TodoItem { checked: true, title: "Implement the .slint file".into() },
        TodoItem { checked: true, title: "Do the Rust part".into() },
        TodoItem { checked: false, title: "Make the C++ code".into() },
        TodoItem { checked: false, title: "Write some JavaScript code".into() },
        TodoItem { checked: false, title: "Test the application".into() },
        TodoItem { checked: false, title: "Ship to customer".into() },
        TodoItem { checked: false, title: "???".into() },
        TodoItem { checked: false, title: "Profit".into() },
    ]));

    let main_window = MainWindow::new();
    main_window.on_todo_added({
        let todo_model = todo_model.clone();
        move |text| todo_model.push(TodoItem { checked: false, title: text })
    });
    main_window.on_remove_done({
        let todo_model = todo_model.clone();
        move || {
            let mut offset = 0;
            for i in 0..todo_model.row_count() {
                if todo_model.row_data(i - offset).unwrap().checked {
                    todo_model.remove(i - offset);
                    offset += 1;
                }
            }
        }
    });

    let weak_window = main_window.as_weak();
    main_window.on_popup_confirmed(move || {
        let window = weak_window.unwrap();
        window.hide();
    });

    {
        let weak_window = main_window.as_weak();
        let todo_model = todo_model.clone();
        main_window.window().on_close_requested(move || {
            let window = weak_window.unwrap();

            if todo_model.iter().any(|t| !t.checked) {
                window.invoke_show_confirm_popup();
                slint::CloseRequestResponse::KeepWindowShown
            } else {
                slint::CloseRequestResponse::HideWindow
            }
        });
    }

    main_window.on_apply_sorting_and_filtering({
        let weak_window = main_window.as_weak();
        let todo_model = todo_model.clone();

        move || {
            let window = weak_window.unwrap();
            window.set_todo_model(todo_model.clone().into());

            if window.get_hide_done_items() {
                window.set_todo_model(
                    Rc::new(FilterModel::new(window.get_todo_model(), |e| !e.checked)).into(),
                );
            }

            if window.get_is_sort_by_name() {
                window.set_todo_model(
                    Rc::new(SortModel::new(window.get_todo_model(), |lhs, rhs| {
                        lhs.title.to_lowercase().cmp(&rhs.title.to_lowercase())
                    }))
                    .into(),
                );
            }
        }
    });

    main_window.set_show_header(true);
    main_window.set_todo_model(todo_model.into());

    main_window.run();
}
