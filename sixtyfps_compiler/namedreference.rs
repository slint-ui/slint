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
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::{Rc, Weak};

use crate::langtype::Type;
use crate::object_tree::{Element, ElementRc};

/// Reference to a property or callback of a given name within an element.
#[derive(Clone)]
pub struct NamedReference(Rc<NamedReferenceInner>);

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
        pretty_print_element_ref(f, &self.0.element)?;
        write!(f, ".{}", self.0.name)
    }
}

impl NamedReference {
    pub fn new(element: &ElementRc, name: &str) -> Self {
        Self(NamedReferenceInner::from_name(element, name))
    }
    pub fn name(&self) -> &str {
        &self.0.name
    }
    pub fn element(&self) -> ElementRc {
        self.0.element.upgrade().unwrap()
    }
    pub fn ty(&self) -> Type {
        self.element().borrow().lookup_property(self.name()).property_type
    }
}

impl Eq for NamedReference {}

impl PartialEq for NamedReference {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Hash for NamedReference {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state);
    }
}

pub struct NamedReferenceInner {
    /// The element.
    pub element: Weak<RefCell<Element>>,
    /// The property name
    pub name: String,
}

impl NamedReferenceInner {
    fn check_invariant(&self) {
        debug_assert!(std::ptr::eq(
            self as *const Self,
            Rc::as_ptr(
                &self.element.upgrade().unwrap().borrow().named_references.0.borrow()[&self.name]
            )
        ))
    }

    pub fn from_name(element: &ElementRc, name: &str) -> Rc<Self> {
        let result = element
            .borrow()
            .named_references
            .0
            .borrow_mut()
            .entry(name.to_owned())
            .or_insert_with_key(|name| {
                Rc::new(Self { element: Rc::downgrade(element), name: name.clone() })
            })
            .clone();
        result.check_invariant();
        result
    }
}

/// Must be put inside the Element and owns all the NamedReferenceInner
#[derive(Default)]
pub struct NamedReferenceContainer(RefCell<HashMap<String, Rc<NamedReferenceInner>>>);
