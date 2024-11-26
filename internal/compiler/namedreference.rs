// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains the [`NamedReference`] and its helper
*/

use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::{Rc, Weak};

use crate::langtype::{ElementType, Type};
use crate::object_tree::{Element, ElementRc, PropertyAnalysis, PropertyVisibility};

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
    pub fn new(element: &ElementRc, name: SmolStr) -> Self {
        Self(NamedReferenceInner::from_name(element, name))
    }
    pub(crate) fn snapshot(&self, snapshotter: &mut crate::typeloader::Snapshotter) -> Self {
        NamedReference(Rc::new(self.0.snapshot(snapshotter)))
    }
    pub fn name(&self) -> &SmolStr {
        &self.0.name
    }
    #[track_caller]
    pub fn element(&self) -> ElementRc {
        self.0.element.upgrade().expect("NamedReference to a dead element")
    }
    pub fn ty(&self) -> Type {
        self.element().borrow().lookup_property(self.name()).property_type
    }

    /// return true if the property has a constant value for the lifetime of the program
    pub fn is_constant(&self) -> bool {
        self.is_constant_impl(true)
    }

    /// return true if we know that this property is changed by other means than its own binding
    pub fn is_externally_modified(&self) -> bool {
        !self.is_constant_impl(false)
    }

    /// return true if the property has a constant value for the lifetime of the program
    fn is_constant_impl(&self, mut check_binding: bool) -> bool {
        let mut elem = self.element();
        let e = elem.borrow();
        if let Some(decl) = e.property_declarations.get(self.name()) {
            if decl.expose_in_public_api && decl.visibility != PropertyVisibility::Input {
                // could be set by the public API
                return false;
            }
        }
        if e.property_analysis.borrow().get(self.name()).map_or(false, |a| a.is_set_externally) {
            return false;
        }
        drop(e);

        loop {
            let e = elem.borrow();
            if e.property_analysis.borrow().get(self.name()).map_or(false, |a| a.is_set) {
                // if the property is set somewhere, it is not constant
                return false;
            }

            if let Some(b) = e.bindings.get(self.name()) {
                if check_binding && !b.borrow().analysis.as_ref().map_or(false, |a| a.is_const) {
                    return false;
                }
                if !b.borrow().two_way_bindings.iter().all(|n| n.is_constant()) {
                    return false;
                }
                check_binding = false;
            }
            if let Some(decl) = e.property_declarations.get(self.name()) {
                if let Some(alias) = &decl.is_alias {
                    return alias.is_constant();
                }
                return true;
            }
            match &e.base_type {
                ElementType::Component(c) => {
                    let next = c.root_element.clone();
                    drop(e);
                    elem = next;
                    continue;
                }
                ElementType::Builtin(b) => {
                    return b.properties.get(self.name()).map_or(true, |pi| !pi.is_native_output())
                }
                ElementType::Native(n) => {
                    return n.properties.get(self.name()).map_or(true, |pi| !pi.is_native_output())
                }
                crate::langtype::ElementType::Error | crate::langtype::ElementType::Global => {
                    return true
                }
            }
        }
    }

    /// Mark that this property is set  somewhere in the code
    pub fn mark_as_set(&self) {
        let element = self.element();
        element
            .borrow()
            .property_analysis
            .borrow_mut()
            .entry(self.name().clone())
            .or_default()
            .is_set = true;
        mark_property_set_derived_in_base(element, self.name())
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

struct NamedReferenceInner {
    /// The element.
    element: Weak<RefCell<Element>>,
    /// The property name
    name: SmolStr,
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

    pub fn from_name(element: &ElementRc, name: SmolStr) -> Rc<Self> {
        let elem = element.borrow();
        let mut named_references = elem.named_references.0.borrow_mut();
        let result = if let Some(r) = named_references.get(&name) {
            r.clone()
        } else {
            let r = Rc::new(Self { element: Rc::downgrade(element), name });
            named_references.insert(r.name.clone(), r.clone());
            r
        };
        drop(named_references);
        result.check_invariant();
        result
    }

    pub(crate) fn snapshot(&self, snapshotter: &mut crate::typeloader::Snapshotter) -> Self {
        let element = if let Some(el) = self.element.upgrade() {
            Rc::downgrade(&snapshotter.use_element(&el))
        } else {
            std::rc::Weak::default()
        };

        Self { element, name: self.name.clone() }
    }
}

/// Must be put inside the Element and owns all the NamedReferenceInner
#[derive(Default)]
pub struct NamedReferenceContainer(RefCell<HashMap<SmolStr, Rc<NamedReferenceInner>>>);

impl NamedReferenceContainer {
    /// Returns true if there is at least one NamedReference pointing to the property `name` in this element.
    pub fn is_referenced(&self, name: &str) -> bool {
        if let Some(nri) = self.0.borrow().get(name) {
            // one reference for the hashmap itself
            Rc::strong_count(nri) > 1
        } else {
            false
        }
    }

    pub(crate) fn snapshot(
        &self,
        snapshotter: &mut crate::typeloader::Snapshotter,
    ) -> NamedReferenceContainer {
        let inner = self
            .0
            .borrow()
            .iter()
            .map(|(k, v)| (k.clone(), Rc::new(v.snapshot(snapshotter))))
            .collect();
        NamedReferenceContainer(RefCell::new(inner))
    }
}

/// Mark that a given property is `is_set_externally` in all bases
pub(crate) fn mark_property_set_derived_in_base(mut element: ElementRc, name: &str) {
    loop {
        let next = if let ElementType::Component(c) = &element.borrow().base_type {
            if element.borrow().property_declarations.contains_key(name) {
                return;
            };
            match c.root_element.borrow().property_analysis.borrow_mut().entry(name.into()) {
                std::collections::hash_map::Entry::Occupied(e) if e.get().is_set_externally => {
                    return;
                }
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    e.get_mut().is_set_externally = true;
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(PropertyAnalysis { is_set_externally: true, ..Default::default() });
                }
            }
            c.root_element.clone()
        } else {
            return;
        };
        element = next;
    }
}

/// Mark that a given property is `is_read_externally` in all bases
pub(crate) fn mark_property_read_derived_in_base(mut element: ElementRc, name: &str) {
    loop {
        let next = if let ElementType::Component(c) = &element.borrow().base_type {
            if element.borrow().property_declarations.contains_key(name) {
                return;
            };
            match c.root_element.borrow().property_analysis.borrow_mut().entry(name.into()) {
                std::collections::hash_map::Entry::Occupied(e) if e.get().is_read_externally => {
                    return;
                }
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    e.get_mut().is_read_externally = true;
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(PropertyAnalysis { is_read_externally: true, ..Default::default() });
                }
            }
            c.root_element.clone()
        } else {
            return;
        };
        element = next;
    }
}
