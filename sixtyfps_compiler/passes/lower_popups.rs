/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Passe that compute the layout constraint

use crate::diagnostics::BuildDiagnostics;
use crate::langtype::Type;
use crate::object_tree::*;
use crate::typeregister::TypeRegister;
use std::rc::Rc;

/// Currently this just removes the layout from the tree
pub fn lower_popups(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let window_type = type_register.lookup_element("Window").unwrap();

    recurse_elem_including_sub_components_no_borrow(
        component,
        &None,
        &mut |elem, parent_element: &Option<ElementRc>| {
            let is_popup = elem.borrow().base_type.to_string() == "PopupWindow";
            if is_popup {
                match parent_element {
                    None => diag
                        .push_error("PopupWindow cannot be the top level".into(), &*elem.borrow()),
                    Some(parent_element) => {
                        lower_popup_window(elem, parent_element, &window_type, diag)
                    }
                }
            }
            Some(elem.clone())
        },
    )
}

fn lower_popup_window(
    popup_window_element: &ElementRc,
    parent_element: &ElementRc,
    window_type: &Type,
    diag: &mut BuildDiagnostics,
) {
    let parent_component = parent_element.borrow().enclosing_component.upgrade().unwrap();
    // Because layout constraint which are supposed to be in the popup will not be lowered, the layout lowering should be done after
    debug_assert!(parent_component.layouts.borrow().is_empty());

    // Remove the popup_window_element from its parent
    parent_element.borrow_mut().children.retain(|child| Rc::ptr_eq(child, popup_window_element));

    popup_window_element.borrow_mut().base_type = window_type.clone();

    let comp = Rc::new(Component {
        root_element: popup_window_element.clone(),
        parent_element: Rc::downgrade(parent_element),
        ..Component::default()
    });

    let weak = Rc::downgrade(&comp);
    recurse_elem(&comp.root_element, &(), &mut |e, _| {
        e.borrow_mut().enclosing_component = weak.clone()
    });

    // Throw error when accessing the popup from outside
    // FIXME:
    // - the span is the span of the PopupWindow, that's wrong, we should have the span of the reference
    // - There are other object reference than in the NamedReference
    // - Maybe this should actually be allowed
    visit_all_named_references(&parent_component, &mut |nr| {
        if std::rc::Weak::ptr_eq(&nr.element.upgrade().unwrap().borrow().enclosing_component, &weak)
        {
            diag.push_error(
                "Cannot access the inside of a PopupWindow from enclosing component".into(),
                &*popup_window_element.borrow(),
            );
        }
    });

    parent_component.popup_windows.borrow_mut().push(PopupWindow { component: comp });
}
