// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!
Make sure that the Repeated expression are just components without any children
 */

use crate::expression_tree::{Expression, NamedReference};
use crate::langtype::Type;
use crate::object_tree::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn process_repeater_components(component: &Rc<Component>) {
    create_repeater_components(component);
    adjust_references(component);
}

fn create_repeater_components(component: &Rc<Component>) {
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        let is_listview = match &elem.borrow().repeated {
            Some(r) => r.is_listview.clone(),
            None => return,
        };
        let parent_element = Rc::downgrade(elem);
        let mut elem = elem.borrow_mut();

        let comp = Rc::new(Component {
            root_element: Rc::new(RefCell::new(Element {
                id: elem.id.clone(),
                base_type: std::mem::take(&mut elem.base_type),
                bindings: std::mem::take(&mut elem.bindings),
                property_analysis: std::mem::take(&mut elem.property_analysis),
                children: std::mem::take(&mut elem.children),
                property_declarations: std::mem::take(&mut elem.property_declarations),
                named_references: Default::default(),
                repeated: None,
                node: elem.node.clone(),
                enclosing_component: Default::default(),
                states: std::mem::take(&mut elem.states),
                transitions: std::mem::take(&mut elem.transitions),
                child_of_layout: elem.child_of_layout || is_listview.is_some(),
                layout_info_prop: elem.layout_info_prop.take(),
                is_flickable_viewport: elem.is_flickable_viewport,
                item_index: Default::default(), // Not determined yet
            })),
            parent_element,
            ..Component::default()
        });

        if let Some(listview) = is_listview {
            if !comp.root_element.borrow().bindings.contains_key("height") {
                let preferred = Expression::PropertyReference(NamedReference::new(
                    &comp.root_element,
                    "preferred-height",
                ));
                comp.root_element
                    .borrow_mut()
                    .bindings
                    .insert("height".into(), RefCell::new(preferred.into()));
            }
            if !comp.root_element.borrow().bindings.contains_key("width") {
                comp.root_element.borrow_mut().bindings.insert(
                    "width".into(),
                    RefCell::new(Expression::PropertyReference(listview.listview_width).into()),
                );
            }

            comp.root_element
                .borrow()
                .property_analysis
                .borrow_mut()
                .entry("y".into())
                .or_default()
                .is_set_externally = true;
        }

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
    visit_all_named_references(comp, &mut |nr| {
        if nr.name() == "$model" {
            return;
        }
        let e = nr.element();
        if e.borrow().repeated.is_some() {
            if let Type::Component(c) = e.borrow().base_type.clone() {
                *nr = NamedReference::new(&c.root_element, nr.name())
            };
        }
    });
    // Transform any references to the repeated element to refer to the root of each instance.
    visit_all_expressions(comp, |expr, _| {
        expr.visit_recursive_mut(&mut |expr| {
            if let Expression::ElementReference(ref mut element_ref) = expr {
                if let Some(repeater_element) =
                    element_ref.upgrade().filter(|e| e.borrow().repeated.is_some())
                {
                    let inner_element =
                        repeater_element.borrow().base_type.as_component().root_element.clone();
                    *element_ref = Rc::downgrade(&inner_element);
                }
            }
        })
    });
}
