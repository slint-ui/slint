/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
Make sure that the Repeated expression are just components without any chilodren
 */

use crate::expression_tree::NamedReference;
use crate::object_tree::*;
use crate::typeregister::Type;
use std::{cell::RefCell, rc::Rc};

pub fn process_repeater_components(component: &Rc<Component>) {
    create_repeater_components(component);
    adjust_references(component);
}

fn create_repeater_components(component: &Rc<Component>) {
    // Because layout constraint which are supposed to be in the repeater will not be lowered
    debug_assert!(component.layout_constraints.borrow().is_empty());

    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        if elem.borrow().repeated.is_none() {
            return;
        }
        let parent_element = Rc::downgrade(elem);
        let mut elem = elem.borrow_mut();

        let comp = Rc::new(Component {
            root_element: Rc::new(RefCell::new(Element {
                id: elem.id.clone(),
                base_type: std::mem::take(&mut elem.base_type),
                bindings: std::mem::take(&mut elem.bindings),
                children: std::mem::take(&mut elem.children),
                property_declarations: std::mem::take(&mut elem.property_declarations),
                property_animations: std::mem::take(&mut elem.property_animations),
                repeated: None,
                node: elem.node.clone(),
                enclosing_component: Default::default(),
                states: std::mem::take(&mut elem.states),
                transitions: std::mem::take(&mut elem.transitions),
                child_of_layout: elem.child_of_layout,
            })),
            parent_element,
            ..Component::default()
        });

        let weak = Rc::downgrade(&comp);
        recurse_elem(&comp.root_element, &(), &mut |e, _| {
            e.borrow_mut().enclosing_component = weak.clone()
        });
        create_repeater_components(&comp);
        elem.base_type = Type::Component(comp);
    });
}

/// Make sure that references to property within the repeated element actually point to the reference
/// to the root of the newly created component
fn adjust_references(comp: &Rc<Component>) {
    recurse_elem(&comp.root_element, &(), &mut |elem, _| {
        visit_all_named_references(elem, |NamedReference { element, name }| {
            if name == "$model" {
                return;
            }
            let e = element.upgrade().unwrap();
            if e.borrow().repeated.is_some() {
                if let Type::Component(c) = e.borrow().base_type.clone() {
                    *element = Rc::downgrade(&c.root_element);
                };
            }
        });
        if let Type::Component(c) = elem.borrow().base_type.clone() {
            adjust_references(&c);
        }
    });
}
