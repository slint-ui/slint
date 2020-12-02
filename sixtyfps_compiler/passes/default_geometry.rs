/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! Set the width and height of Rectangle, TouchArea, ... to 100%.

use std::rc::Rc;

use crate::expression_tree::{Expression, NamedReference};
use crate::langtype::Type;
use crate::object_tree::{Component, ElementRc};

pub fn default_geometry(root_component: &Rc<Component>) {
    crate::object_tree::recurse_elem_including_sub_components(
        &root_component,
        &None,
        &mut |elem: &ElementRc, parent: &Option<ElementRc>| {
            if let Some(parent) = parent {
                let should_expand = if let Type::Builtin(b) = &elem.borrow().base_type {
                    b.expands_to_parent_geometry
                } else {
                    false
                };
                if should_expand && !elem.borrow().child_of_layout {
                    make_default_100(elem, parent, "width");
                    make_default_100(elem, parent, "height");
                }
            }
            Some(elem.clone())
        },
    )
}

fn make_default_100(elem: &ElementRc, parent_element: &ElementRc, property: &str) {
    if parent_element.borrow().lookup_property(property) != Type::Length {
        return;
    }
    elem.borrow_mut().bindings.entry(property.into()).or_insert_with(|| {
        Expression::PropertyReference(NamedReference::new(parent_element, property)).into()
    });
}
