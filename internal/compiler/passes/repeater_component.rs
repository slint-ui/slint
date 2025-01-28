// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
Make sure that the Repeated expression are just components without any children
 */

use crate::expression_tree::{Expression, NamedReference};
use crate::langtype::ElementType;
use crate::object_tree::*;
use smol_str::SmolStr;
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

        if matches!(&elem.base_type, ElementType::Component(c) if c.parent_element.upgrade().is_some())
        {
            debug_assert!(std::rc::Weak::ptr_eq(
                &parent_element,
                &elem.base_type.as_component().parent_element
            ));
            // Already processed (can happen if a component is both used and exported root)
            return;
        }

        let comp = Rc::new(Component {
            root_element: Rc::new(RefCell::new(Element {
                id: elem.id.clone(),
                base_type: std::mem::take(&mut elem.base_type),
                bindings: std::mem::take(&mut elem.bindings),
                change_callbacks: std::mem::take(&mut elem.change_callbacks),
                property_analysis: std::mem::take(&mut elem.property_analysis),
                children: std::mem::take(&mut elem.children),
                property_declarations: std::mem::take(&mut elem.property_declarations),
                named_references: Default::default(),
                repeated: None,
                is_component_placeholder: false,
                debug: elem.debug.clone(),
                enclosing_component: Default::default(),
                states: std::mem::take(&mut elem.states),
                transitions: std::mem::take(&mut elem.transitions),
                child_of_layout: elem.child_of_layout || is_listview.is_some(),
                layout_info_prop: elem.layout_info_prop.take(),
                default_fill_parent: elem.default_fill_parent,
                accessibility_props: std::mem::take(&mut elem.accessibility_props),
                geometry_props: elem.geometry_props.clone(),
                is_flickable_viewport: elem.is_flickable_viewport,
                has_popup_child: elem.has_popup_child,
                item_index: Default::default(), // Not determined yet
                item_index_of_first_children: Default::default(),
                is_legacy_syntax: elem.is_legacy_syntax,
                inline_depth: 0,
            })),
            parent_element,
            ..Component::default()
        });

        if let Some(listview) = is_listview {
            if !comp.root_element.borrow().is_binding_set("height", false) {
                let preferred = Expression::PropertyReference(NamedReference::new(
                    &comp.root_element,
                    SmolStr::new_static("preferred-height"),
                ));
                comp.root_element
                    .borrow_mut()
                    .bindings
                    .insert("height".into(), RefCell::new(preferred.into()));
            }
            if !comp.root_element.borrow().is_binding_set("width", false) {
                comp.root_element.borrow_mut().bindings.insert(
                    "width".into(),
                    RefCell::new(Expression::PropertyReference(listview.listview_width).into()),
                );
            }
        }

        let weak = Rc::downgrade(&comp);
        recurse_elem(&comp.root_element, &(), &mut |e, _| {
            e.borrow_mut().enclosing_component = weak.clone()
        });
        create_repeater_components(&comp);
        elem.base_type = ElementType::Component(comp);
    });

    for p in component.popup_windows.borrow().iter() {
        create_repeater_components(&p.component);
    }
    for c in component.menu_item_tree.borrow().iter() {
        create_repeater_components(c);
    }
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
            if let ElementType::Component(c) = e.borrow().base_type.clone() {
                *nr = NamedReference::new(&c.root_element, nr.name().clone())
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
