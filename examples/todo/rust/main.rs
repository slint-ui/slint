/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use core::pin;
use pin::Pin;
use sixtyfps::Model;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

sixtyfps::include_modules!();

struct TodoModel {
    backing_model: sixtyfps::VecModel<TodoItem>,
    main_window: sixtyfps::re_exports::PinWeak<MainWindow>,
}

impl sixtyfps::Model for TodoModel {
    type Data = TodoItem;

    fn row_count(&self) -> usize {
        self.backing_model.row_count()
    }

    fn row_data(&self, row: usize) -> Self::Data {
        self.backing_model.row_data(row)
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let previous_checked = self.backing_model.row_data(row).checked;
        let new_checked = data.checked;
        match (previous_checked, new_checked) {
            (true, false) => self.adjust_todo_count(1),
            (false, true) => self.adjust_todo_count(-1),
            _ => {}
        };

        self.backing_model.set_row_data(row, data)
    }

    fn attach_peer(&self, peer: sixtyfps::ModelPeer) {
        self.backing_model.attach_peer(peer)
    }
}

impl TodoModel {
    fn new(data: Vec<TodoItem>, main_window: &Pin<Rc<MainWindow>>) -> Self {
        let todo_left_count =
            data.iter().fold(0, |count, item| if item.checked { count } else { count + 1 });
        main_window.as_ref().set_todo_left(todo_left_count);
        Self {
            backing_model: data.into(),
            main_window: sixtyfps::re_exports::PinWeak::downgrade(main_window.clone()),
        }
    }

    pub fn push(&self, value: TodoItem) {
        if !value.checked {
            self.adjust_todo_count(1);
        }
        self.backing_model.push(value)
    }

    pub fn remove(&self, index: usize) {
        if !self.backing_model.row_data(index).checked {
            self.adjust_todo_count(-1)
        }
        self.backing_model.remove(index)
    }

    fn adjust_todo_count(&self, adjustment: i32) {
        let main_window = self.main_window.upgrade().unwrap();
        main_window.as_ref().set_todo_left(main_window.as_ref().get_todo_left() + adjustment);
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let main_window = MainWindow::new();

    let todo_model = Rc::new(TodoModel::new(
        vec![
            TodoItem { checked: true, title: "Implement the .60 file".into() },
            TodoItem { checked: true, title: "Do the Rust part".into() },
            TodoItem { checked: false, title: "Make the C++ code".into() },
            TodoItem { checked: false, title: "Write some JavaScript code".into() },
            TodoItem { checked: false, title: "Test the application".into() },
            TodoItem { checked: false, title: "Ship to customer".into() },
            TodoItem { checked: false, title: "???".into() },
            TodoItem { checked: false, title: "Profit".into() },
        ],
        &main_window,
    ));

    main_window.as_ref().on_todo_added({
        let todo_model = todo_model.clone();
        move |text| todo_model.push(TodoItem { checked: false, title: text })
    });
    main_window.as_ref().on_remove_done({
        let todo_model = todo_model.clone();
        move || {
            let mut offset = 0;
            for i in 0..todo_model.row_count() {
                if todo_model.row_data(i - offset).checked {
                    todo_model.remove(i - offset);
                    offset += 1;
                }
            }
        }
    });

    main_window.as_ref().set_todo_model(Some(todo_model));

    main_window.run();
}
