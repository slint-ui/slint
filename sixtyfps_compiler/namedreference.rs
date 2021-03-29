/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
This module contains the [`NamedReference`] and its helper
*/

use std::cell::RefCell;
use std::hash::Hash;
use std::rc::{Rc, Weak};

use crate::object_tree::{Element, ElementRc};

/// Reference to a property or callback of a given name within an element.
#[derive(Clone)]
pub struct NamedReference {
    pub element: Weak<RefCell<Element>>,
    pub name: String,
}

pub fn pretty_print_element_ref(
    f: &mut dyn std::fmt::Write,
    element: &Weak<RefCell<Element>>,
) -> std::fmt::Result {
    match element.upgrade() {
        Some(e) => match e.try_borrow() {
            Ok(el) => write!(f, "{}", el.id),
            Err(_) => write!(f, "<borrowed>"),
        },
        None => write!(f, "<null>"),
    }
}

impl std::fmt::Debug for NamedReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        pretty_print_element_ref(f, &self.element)?;
        write!(f, ".{}", self.name)
    }
}

impl NamedReference {
    pub fn new(element: &ElementRc, name: &str) -> Self {
        Self { element: Rc::downgrade(element), name: name.to_owned() }
    }
}

impl Eq for NamedReference {}

impl PartialEq for NamedReference {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && Weak::ptr_eq(&self.element, &other.element)
    }
}

impl Hash for NamedReference {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.element.as_ptr().hash(state);
    }
}

/*
pub struct PropertyReference {
    /// The element.
    pub element: Weak<Element>,
    /// The property name
    pub name: String,
}

impl PropertyReference {
    fn check_invarient(&self) {
        debug_assert!(std::ptr::eq(
            self as *const PropertyReference,
            Rc::as_ptr(&self.element.upgrade().unwrap().property_references[&self.name])
        ))
    }

    pub fn from_name(element: &ElementRc, name: String) -> Rc<Self> {
        element
            .borrow_mut()
            .property_references
            .entry(name)
            .or_insert_with_key(|name| Self { element: Rc::downgrade(element), name: name.clone() })
            .clone()
    }
}
*/

#[derive(Default)]
pub struct NamedReferenceContainer;
