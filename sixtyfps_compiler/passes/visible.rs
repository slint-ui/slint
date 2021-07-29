/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Pass that lowers synthetic `visible` properties to Clip element

use std::cell::RefCell;
use std::rc::Rc;

use crate::expression_tree::{Expression, NamedReference};
use crate::langtype::{NativeClass, Type};
use crate::object_tree::{self, Component, Element, ElementRc};
use crate::typeregister::TypeRegister;

pub fn handle_visible(component: &Rc<Component>, type_register: &TypeRegister) {
    let native_clip = type_register.lookup("Clip").as_builtin().native_class.clone();

    crate::object_tree::recurse_elem_including_sub_components(
        component,
        &(),
        &mut |elem: &ElementRc, _| {
            if elem.borrow().native_class().map_or(false, |n| {
                Rc::ptr_eq(&n, &native_clip) && elem.borrow().id.ends_with("_visibility")
            }) {
                // This is the element we just created. Skip it.
                return;
            }

            let old_children = {
                let mut elem = elem.borrow_mut();
                let new_children = Vec::with_capacity(elem.children.len());
                std::mem::replace(&mut elem.children, new_children)
            };

            let has_visible_binding = |e: &ElementRc| {
                e.borrow().base_type.lookup_property("visible").property_type != Type::Invalid
                    && (e.borrow().bindings.contains_key("visible")
                        || e.borrow()
                            .property_analysis
                            .borrow()
                            .get("visible")
                            .map_or(false, |a| a.is_set))
            };

            for mut child in old_children {
                if child.borrow().repeated.is_some() {
                    let root_elem = child.borrow().base_type.as_component().root_element.clone();
                    if has_visible_binding(&root_elem) {
                        object_tree::inject_element_as_repeated_element(
                            &child,
                            create_visibility_element(&root_elem, &native_clip),
                        )
                    }
                } else if has_visible_binding(&child) {
                    let new_child = create_visibility_element(&child, &native_clip);
                    new_child.borrow_mut().children.push(child);
                    child = new_child;
                }

                elem.borrow_mut().children.push(child);
            }
        },
    );
}

fn create_visibility_element(child: &ElementRc, native_clip: &Rc<NativeClass>) -> ElementRc {
    let element = Element {
        id: format!("{}_visibility", child.borrow().id),
        base_type: Type::Native(native_clip.clone()),
        enclosing_component: child.borrow().enclosing_component.clone(),
        bindings: std::iter::once((
            "clip".to_owned(),
            Expression::UnaryOp {
                sub: Box::new(Expression::PropertyReference(NamedReference::new(child, "visible"))),
                op: '!',
            }
            .into(),
        ))
        .collect(),
        ..Default::default()
    };
    Rc::new(RefCell::new(element))
}
