// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Application-wide state store.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Central store for application state.
/// Persists data across screen push/pop cycles.
pub struct AppStore {
    data: RefCell<HashMap<String, Box<dyn Any>>>,
}

impl AppStore {
    pub fn new() -> Rc<Self> {
        Rc::new(Self { data: RefCell::new(HashMap::new()) })
    }

    /// Get a value from the store, cloning it.
    pub fn get<T: Clone + 'static>(&self, key: &str) -> Option<T> {
        self.data.borrow().get(key)?.downcast_ref::<T>().cloned()
    }

    /// Get a value or return default.
    pub fn get_or_default<T: Clone + Default + 'static>(&self, key: &str) -> T {
        self.get(key).unwrap_or_default()
    }

    /// Set a value in the store.
    pub fn set<T: 'static>(&self, key: &str, value: T) {
        self.data.borrow_mut().insert(key.to_string(), Box::new(value));
    }

    /// Remove a value from the store.
    pub fn remove(&self, key: &str) {
        self.data.borrow_mut().remove(key);
    }

    /// Check if a key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.data.borrow().contains_key(key)
    }
}
