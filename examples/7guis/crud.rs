// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial
use slint::{Model, ModelExt, SharedString, StandardListViewItem, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

slint::slint!(import { MainWindow } from "crud.slint";);

#[derive(Clone)]
struct Name {
    first: String,
    last: String,
}
pub fn main() {
    let main_window = MainWindow::new();

    let prefix = Rc::new(RefCell::new(SharedString::from("")));
    let prefix_for_wrapper = prefix.clone();

    let model = Rc::new(VecModel::from(vec![
        Name { first: "Hans".to_string(), last: "Emil".to_string() },
        Name { first: "Max".to_string(), last: "Mustermann".to_string() },
        Name { first: "Roman".to_string(), last: "Tisch".to_string() },
    ]));

    let filtered_model = Rc::new(
        model
            .clone()
            .map(|n| StandardListViewItem::from(slint::format!("{}, {}", n.last, n.first)))
            .filter(move |e| e.text.starts_with(prefix_for_wrapper.borrow().as_str())),
    );

    main_window.set_names_list(filtered_model.clone().into());

    {
        let model = model.clone();
        main_window.on_createClicked(move |main_window| {
            let new_entry = Name {
                first: main_window.get_name().to_string(),
                last: main_window.get_surname().to_string(),
            };
            model.push(new_entry);
        });
    }

    {
        let model = model.clone();
        let filtered_model = filtered_model.clone();
        main_window.on_updateClicked(move |main_window| {
            let updated_entry = Name {
                first: main_window.get_name().to_string(),
                last: main_window.get_surname().to_string(),
            };

            let row = filtered_model.unfiltered_row(main_window.get_current_item() as usize);
            model.set_row_data(row, updated_entry);
        });
    }

    {
        let model = model.clone();
        let filtered_model = filtered_model.clone();
        main_window.on_deleteClicked(move |main_window| {
            let index = filtered_model.unfiltered_row(main_window.get_current_item() as usize);
            model.remove(index);
        });
    }

    {
        let filtered_model = filtered_model.clone();
        main_window.on_prefixEdited(move |main_window| {
            *prefix.borrow_mut() = main_window.get_prefix();
            filtered_model.reset();
        });
    }

    main_window.run();
}
