// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial
use _7guis::wrapper::*;
use slint::SharedString;
use slint::StandardListViewItem;
use std::cell::RefCell;
use std::rc::Rc;

slint::slint!(import { MainWindow } from "crud.slint";);

struct Name {
    first: String,
    last: String,
}
pub fn main() {
    let main_window = MainWindow::new();

    let prefix = Rc::new(RefCell::new(SharedString::from("")));
    let prefix_for_wrapper = prefix.clone();

    let model = Rc::new(
        VecWrapper::from(vec![
            Name { first: "Hans".to_string(), last: "Emil".to_string() },
            Name { first: "Max".to_string(), last: "Mustermann".to_string() },
            Name { first: "Roman".to_string(), last: "Tisch".to_string() },
        ])
        .map(|n: &Name| StandardListViewItem::from(format!("{}, {}", n.last, n.first).as_str()))
        .filter(move |e| e.text.starts_with(prefix_for_wrapper.borrow().as_str()))
        .build(),
    );

    main_window.set_names_list(model.clone().into());

    {
        let main_window_weak = main_window.as_weak();
        let model = model.clone();
        main_window.on_createClicked(move || {
            let main_window = main_window_weak.unwrap();
            let new_entry = Name {
                first: main_window.get_name().to_string(),
                last: main_window.get_surname().to_string(),
            };
            model.push(new_entry);
        });
    }

    {
        let main_window_weak = main_window.as_weak();
        let model = model.clone();
        main_window.on_updateClicked(move || {
            let main_window = main_window_weak.unwrap();
            let index = model.index_for_row(main_window.get_current_item() as usize);
            let updated_entry = Name {
                first: main_window.get_name().to_string(),
                last: main_window.get_surname().to_string(),
            };

            model.get_mut(index).set(updated_entry);
        });
    }

    {
        let main_window_weak = main_window.as_weak();
        let model = model.clone();
        main_window.on_deleteClicked(move || {
            let main_window = main_window_weak.unwrap();
            let index = model.index_for_row(main_window.get_current_item() as usize);

            model.remove(index);
        });
    }

    {
        let main_window_weak = main_window.as_weak();
        let model = model.clone();
        main_window.on_prefixEdited(move || {
            let main_window = main_window_weak.unwrap();
            *prefix.borrow_mut() = main_window.get_prefix();
            model.apply_filters();
        });
    }

    main_window.run();
}
