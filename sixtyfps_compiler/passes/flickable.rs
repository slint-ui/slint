/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! Flickable pass
//!
//! The Flickable element is special in the sense that it has a viewport
//! which is not exposed. This passes create the viewport and fixes all property access

use std::cell::RefCell;
use std::rc::Rc;

use crate::expression_tree::{Expression, NamedReference};
use crate::langtype::Type;
use crate::object_tree::{Component, Element, ElementRc};
use crate::typeregister::TypeRegister;

pub fn handle_flickable(root_component: &Rc<Component>, tr: &TypeRegister) -> () {
    let mut native_rect = tr.lookup("Rectangle").as_builtin().native_class.clone();
    while let Some(p) = native_rect.parent.clone() {
        native_rect = p;
    }
    crate::object_tree::recurse_elem_including_sub_components(
        &root_component,
        &(),
        &mut |elem: &ElementRc, _| {
            if !matches!(elem.borrow().native_class(), Some(n) if n.class_name == "Flickable") {
                return;
            }

            let mut flickable = elem.borrow_mut();
            let flickable = &mut *flickable;

            let viewport = Rc::new(RefCell::new(Element {
                id: format!("{}_viewport", flickable.id),
                base_type: Type::Native(native_rect.clone()),
                children: std::mem::take(&mut flickable.children),
                enclosing_component: flickable.enclosing_component.clone(),
                is_flickable_viewport: true,
                ..Element::default()
            }));

            // Create aliases.  All these aliases should be removed by the alias optimisation pass
            for (prop, ty) in &flickable.base_type.as_builtin().properties {
                if let Some(vp_prop) = prop.strip_prefix("viewport_") {
                    let nr = NamedReference::new(&viewport, vp_prop);
                    flickable.property_declarations.insert(prop.to_owned(), ty.clone().into());
                    match flickable.bindings.entry(prop.to_owned()) {
                        std::collections::hash_map::Entry::Occupied(entry) => {
                            let entry = entry.into_mut();
                            entry.expression = Expression::TwoWayBinding(
                                nr,
                                Some(Box::new(std::mem::take(&mut entry.expression))),
                            )
                        }
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            entry.insert(Expression::TwoWayBinding(nr, None).into());
                        }
                    }
                }
            }

            flickable.children.push(viewport);
        },
    )
}
