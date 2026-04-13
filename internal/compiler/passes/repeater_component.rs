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
use std::rc::{Rc, Weak};

pub fn process_repeater_components(component: &Rc<Component>) {
    create_repeater_components(component);
    adjust_references(component);
}

fn create_repeater_components(component: &Rc<Component>) {
    recurse_elem(&component.root_element, &(), &mut |original_elem_rc, _| {
        let is_listview = match &original_elem_rc.borrow().repeated {
            Some(r) => r.is_listview.clone(),
            None => return,
        };
        let original_elem_as_weak = Rc::downgrade(original_elem_rc);
        let mut original_elem = original_elem_rc.borrow_mut();

        if matches!(&original_elem.base_type, ElementType::Component(c) if c.parent_element().is_some())
        {
            debug_assert!(std::rc::Weak::ptr_eq(
                &original_elem_as_weak,
                &*original_elem.base_type.as_component().parent_element.borrow()
            ));
            // Already processed (can happen if a component is both used and exported root)
            return;
        }

        let repeated_component = Rc::new(Component {
            root_element: Rc::new(RefCell::new(Element {
                id: original_elem.id.clone(),
                base_type: std::mem::take(&mut original_elem.base_type),
                bindings: std::mem::take(&mut original_elem.bindings),
                change_callbacks: std::mem::take(&mut original_elem.change_callbacks),
                property_analysis: std::mem::take(&mut original_elem.property_analysis),
                children: std::mem::take(&mut original_elem.children),
                property_declarations: std::mem::take(&mut original_elem.property_declarations),
                named_references: Default::default(),
                repeated: None,
                is_component_placeholder: false,
                debug: original_elem.debug.clone(),
                enclosing_component: Default::default(),
                states: std::mem::take(&mut original_elem.states),
                transitions: std::mem::take(&mut original_elem.transitions),
                child_of_layout: original_elem.child_of_layout || is_listview.is_some(),
                layout_info_prop: original_elem.layout_info_prop.take(),
                default_fill_parent: original_elem.default_fill_parent,
                accessibility_props: std::mem::take(&mut original_elem.accessibility_props),
                geometry_props: original_elem.geometry_props.clone(),
                is_flickable_viewport: original_elem.is_flickable_viewport,
                has_popup_child: original_elem.has_popup_child,
                item_index: Default::default(), // Not determined yet
                item_index_of_first_children: Default::default(),
                is_legacy_syntax: original_elem.is_legacy_syntax,
                inline_depth: 0,
                grid_layout_cell: original_elem.grid_layout_cell.clone(),
            })),
            parent_element: RefCell::new(Weak::clone(&original_elem_as_weak)),
            ..Component::default()
        });

        if let Some(listview) = is_listview {
            if !repeated_component.root_element.borrow().is_binding_set("height", false) {
                let preferred = Expression::PropertyReference(NamedReference::new(
                    &repeated_component.root_element,
                    SmolStr::new_static("preferred-height"),
                ));
                repeated_component
                    .root_element
                    .borrow_mut()
                    .bindings
                    .insert("height".into(), RefCell::new(preferred.into()));
            }
            if !repeated_component.root_element.borrow().is_binding_set("width", false) {
                repeated_component.root_element.borrow_mut().bindings.insert(
                    "width".into(),
                    RefCell::new(Expression::PropertyReference(listview.listview_width).into()),
                );
            }
        }

        let repeated_component_weak = Rc::downgrade(&repeated_component);
        recurse_elem(&repeated_component.root_element, &(), &mut |e, _| {
            e.borrow_mut().enclosing_component = repeated_component_weak.clone()
        });
        // Remove the mutable borrow from the RefCell, so that we can later compare it with the parent_element of the menu items
        drop(original_elem);

        // Move all the menus that belong to the newly created component
        // Could use Vec::extract_if if MSRV >= 1.87
        component.menu_item_tree.borrow_mut().retain(|menu_item| {
            let mut parent_elem = menu_item.parent_element.borrow_mut();

            // When parent_element IS the element being split, update the parent_elem
            // to point to the new sub-component's root.
            if Weak::ptr_eq(&parent_elem, &original_elem_as_weak) {
                *parent_elem = Rc::downgrade(&repeated_component.root_element);
            }

            let enclosing_component =
                parent_elem.upgrade().unwrap().borrow().enclosing_component.clone();
            let should_move = Weak::ptr_eq(&enclosing_component, &repeated_component_weak);
            if should_move {
                repeated_component.menu_item_tree.borrow_mut().push(menu_item.clone());
                false
            } else {
                true
            }
        });

        create_repeater_components(&repeated_component);
        original_elem_rc.borrow_mut().base_type = ElementType::Component(repeated_component);
    });

    for p in component.popup_windows.borrow().iter() {
        create_repeater_components(&p.component);
    }
    for c in component.menu_item_tree.borrow().iter() {
        create_repeater_components(c);
    }
}

/// Make sure that references to properties within the repeated element actually point to the reference
/// to the root of the newly created component
pub fn adjust_references(component: &Rc<Component>) {
    visit_all_named_references(component, &mut |reference| {
        if reference.name() == "$model" {
            return;
        }
        let referred_element = reference.element();
        if referred_element.borrow().repeated.is_some()
            && let ElementType::Component(created_component) =
                referred_element.borrow().base_type.clone()
        {
            *reference =
                NamedReference::new(&created_component.root_element, reference.name().clone())
        };
    });
    // Transform any references to the repeated element to refer to the root of each instance.
    visit_all_expressions(component, |expr, _| {
        expr.visit_recursive_mut(&mut |expr| {
            if let Expression::ElementReference(element_ref) = expr
                && let Some(repeater_element) =
                    element_ref.upgrade().filter(|e| e.borrow().repeated.is_some())
            {
                let inner_element =
                    repeater_element.borrow().base_type.as_component().root_element.clone();
                *element_ref = Rc::downgrade(&inner_element);
            }
        })
    });
}
