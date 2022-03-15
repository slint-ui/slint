// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial
use slint::{Model, ModelNotify, ModelTracker, SharedString};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

slint::slint!(import { MainWindow } from "crud.slint";);

#[derive(Clone)]
struct UniqueString {
    string: SharedString,
    id: u32,
}

struct InnerFilteredStringModel {
    array: Vec<UniqueString>,
    next_id: u32,

    filtered_array: Vec<UniqueString>,
    filtered_ids: HashSet<u32>,

    prefix: SharedString,
}

struct FilteredStringModel {
    inner: RefCell<InnerFilteredStringModel>,
    notify: ModelNotify,
}

impl FilteredStringModel {
    fn filter_by_prefix(&self, prefix: SharedString) {
        let mut inner = self.inner.borrow_mut();

        enum Change {
            Removed { index: usize },
            Added { index: usize, us: UniqueString },
        }

        let mut changes = Vec::new();
        {
            let mut old_iter = inner.filtered_array.iter().enumerate().peekable();
            let mut new_iter =
                inner.array.iter().filter(|us| us.string.starts_with(prefix.as_str())).peekable();

            loop {
                if let Some(&(index, us_old)) = old_iter.peek() {
                    if let Some(&us) = new_iter.peek() {
                        if us_old.id != us.id {
                            if inner.filtered_ids.contains(&us.id) {
                                old_iter.next();
                                changes.push(Change::Removed { index });
                            } else {
                                let new_us = new_iter.next().unwrap();
                                changes.push(Change::Added { index, us: new_us.clone() });
                            }
                        } else {
                            old_iter.next();
                            new_iter.next();
                        }
                    } else {
                        changes.push(Change::Removed { index });
                        old_iter.next();
                    }
                } else {
                    let index = inner.filtered_array.len();
                    for new_us in new_iter {
                        changes.push(Change::Added { index, us: new_us.clone() });
                    }
                    break;
                }
            }
        }

        let mut bias = 0_i32;
        for change in changes {
            match change {
                Change::Removed { index } => {
                    let index = (index as i32 + bias) as usize;
                    let id = inner.filtered_array.remove(index).id;
                    inner.filtered_ids.remove(&id);
                    self.notify.row_removed(index, 1);
                    bias -= 1;
                }
                Change::Added { index, us } => {
                    let index = (index as i32 + bias) as usize;
                    inner.filtered_ids.insert(us.id);
                    inner.filtered_array.insert(index, us);
                    self.notify.row_added(index, 1);
                    bias += 1
                }
            }
        }
        inner.prefix = prefix;
    }

    fn push(&self, string: SharedString) {
        let mut inner = self.inner.borrow_mut();
        let us = UniqueString { string: string.clone(), id: inner.next_id };
        inner.array.push(us.clone());
        inner.next_id += 1;

        if string.starts_with(inner.prefix.as_str()) {
            inner.filtered_ids.insert(us.id);
            inner.filtered_array.push(us);
            self.notify.row_added(inner.filtered_array.len() - 1, 1);
        }
    }

    fn remove(&self, index: usize) {
        let mut inner = self.inner.borrow_mut();

        let id = inner.filtered_array.remove(index).id;
        self.notify.row_removed(index, 1);
        inner.filtered_ids.remove(&id);

        let unfiltered_index = inner.array.iter().position(|us| us.id == id).unwrap();
        inner.array.remove(unfiltered_index);
    }
}

impl Model for FilteredStringModel {
    type Data = slint::StandardListViewItem;

    fn row_count(&self) -> usize {
        self.inner.borrow().filtered_array.len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.inner
            .borrow()
            .filtered_array
            .get(row)
            .map(|us| slint::StandardListViewItem::from(us.string.clone()))
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl From<Vec<SharedString>> for FilteredStringModel {
    fn from(array: Vec<SharedString>) -> Self {
        let array: Vec<UniqueString> = array
            .iter()
            .zip(0_u32..)
            .map(|(s, i)| UniqueString { string: s.clone(), id: i })
            .collect();
        FilteredStringModel {
            inner: RefCell::new(InnerFilteredStringModel {
                array: array.clone(),
                next_id: array.len() as u32,
                filtered_ids: (0_u32..array.len() as u32).collect(),
                filtered_array: array,
                prefix: "".into(),
            }),
            notify: Default::default(),
        }
    }
}

pub fn main() {
    let main_window = MainWindow::new();

    let model = Rc::new(FilteredStringModel::from(vec![
        "Emil, Hans".into(),
        "Mustermann, Max".into(),
        "Tisch, Roman".into(),
    ]));
    main_window.set_names_list(model.clone().into());

    {
        let main_window_weak = main_window.as_weak();
        let model = model.clone();
        main_window.on_createClicked(move || {
            let main_window = main_window_weak.unwrap();
            let new_entry = main_window.get_surname() + ", " + main_window.get_name().as_str();

            model.push(new_entry.into());
        });
    }

    {
        let main_window_weak = main_window.as_weak();
        let model = model.clone();
        main_window.on_updateClicked(move || {
            let main_window = main_window_weak.unwrap();
            let index = main_window.get_current_item() as usize;
            let entry = main_window.get_surname() + ", " + main_window.get_name().as_str();

            model.set_row_data(index, entry.into());
        });
    }

    {
        let main_window_weak = main_window.as_weak();
        let model = model.clone();
        main_window.on_deleteClicked(move || {
            let main_window = main_window_weak.unwrap();
            let index = main_window.get_current_item() as usize;

            model.remove(index);
        });
    }

    {
        let main_window_weak = main_window.as_weak();
        let model = model.clone();
        main_window.on_prefixEdited(move || {
            let main_window = main_window_weak.unwrap();
            let prefix = main_window.get_prefix();

            model.filter_by_prefix(prefix);
        });
    }

    main_window.run();
}
