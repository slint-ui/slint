// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::cell::RefCell;
use slint::{FilterModel, Model, SharedString};
use std::rc::{Rc, Weak};

trait SearchModelInnerErased<Data> {
    fn set_search_text(&self, text: SharedString);
    fn model(&self) -> &dyn Model<Data = Data>;
    fn source(&self) -> &dyn Model<Data = Data>;
    fn search_text(&self) -> SharedString;
}

struct SearchModelInner<M, F>
where
    M: Model + 'static,
    F: Fn(&M::Data) -> bool + 'static,
{
    model: FilterModel<M, F>,
    filter_text: RefCell<SharedString>,
}

impl<M, F> SearchModelInnerErased<M::Data> for SearchModelInner<M, F>
where
    M: Model + 'static,
    F: Fn(&M::Data) -> bool + 'static,
{
    fn set_search_text(&self, text: SharedString) {
        self.filter_text.replace(text);
        self.model.reset();
    }
    fn model(&self) -> &dyn Model<Data = M::Data> {
        &self.model
    }
    fn source(&self) -> &dyn Model<Data = M::Data> {
        self.model.source_model()
    }
    fn search_text(&self) -> SharedString {
        self.filter_text.borrow().clone()
    }
}

pub struct SearchModel<Data> {
    inner: Rc<dyn SearchModelInnerErased<Data>>,
}

impl<Data: 'static> Model for SearchModel<Data> {
    type Data = Data;

    fn row_count(&self) -> usize {
        self.inner.model().row_count()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.inner.model().row_data(row)
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        self.inner.model().model_tracker()
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        self.inner.model().set_row_data(row, data)
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl<Data: 'static> SearchModel<Data> {
    pub fn new(
        model: impl Model<Data = Data> + 'static,
        search_fn: impl Fn(&Data, &SharedString) -> bool + 'static,
    ) -> Self {
        let inner = Rc::new_cyclic(move |weak: &Weak<SearchModelInner<_, _>>| {
            let weak = weak.clone() as Weak<dyn SearchModelInnerErased<Data>>;
            SearchModelInner {
                model: FilterModel::new(model, move |data| {
                    weak.upgrade().is_none_or(|inner| search_fn(data, &inner.search_text()))
                }),
                filter_text: Default::default(),
            }
        });
        Self { inner }
    }

    pub fn set_search_text(&self, text: SharedString) {
        self.inner.set_search_text(text);
    }

    pub fn search_text(&self) -> SharedString {
        self.inner.search_text()
    }

    pub fn source_model(&self) -> &dyn Model<Data = Data> {
        self.inner.source()
    }
}

/// Return true if the given data contains the search text, case insensitive search
pub fn contains(data: &str, search_text: &str) -> bool {
    if search_text.is_empty() {
        return true;
    }
    let mut it = data.chars();
    loop {
        let mut cloned = it.clone();
        if search_text.chars().all(|a| {
            cloned.next().is_some_and(|b| {
                nucleo_matcher::chars::normalize(a).to_ascii_lowercase()
                    == nucleo_matcher::chars::normalize(b).to_ascii_lowercase()
            })
        }) {
            return true;
        }
        if it.next().is_none() {
            return false;
        }
    }
}

#[test]
fn test_contains() {
    assert!(contains("Hello", ""));
    assert!(contains("", ""));
    assert!(!contains("", "Something"));
    assert!(contains("Hello", "Hello"));
    assert!(contains("Héllo Wold", "Hello"));
    assert!(contains("AbcHello Wold", "héllo"));
    assert!(contains("AXyzXyzXyzabcA", "xyzabc"));
    assert!(!contains("Hello", "Foo"));
    assert!(!contains("FOXFO", "Foo"));
    assert!(!contains("FO", "Foo"));
}
