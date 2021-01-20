/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! Pass that takes the mouse-clicked, key-pressed, key-release event
//! and generate the FocusScope and TouchArea

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::Expression;
use crate::object_tree::*;
use crate::typeregister::TypeRegister;
use std::cell::RefCell;
use std::rc::Rc;

pub fn lower_events(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    _diag: &mut BuildDiagnostics,
) {
    recurse_elem_including_sub_components_no_borrow(&component, &(), &mut |elem, _| {
        let mut elem = elem.borrow_mut();

        if let Some(handler) = elem.bindings.remove("mouse_clicked") {
            let mut new_elem = Element {
                id: format!("{}_mouse", elem.id),
                base_type: type_register.lookup_element("TouchArea").unwrap(),
                enclosing_component: elem.enclosing_component.clone(),
                ..Default::default()
            };
            new_elem.bindings.insert("clicked".into(), handler);
            elem.children.push(ElementRc::new(RefCell::new(new_elem)));
        }

        if elem.base_type.to_string() == "FocusScope" {
            return;
        }
        match (elem.bindings.remove("key_pressed"), elem.bindings.remove("key_released")) {
            (None, None) => {}
            (pressed, released) => {
                let mut new_elem = Element {
                    id: format!("{}_keyboard", elem.id),
                    base_type: type_register.lookup_element("FocusScope").unwrap(),
                    enclosing_component: elem.enclosing_component.clone(),
                    ..Default::default()
                };
                if let Some(handler) = pressed {
                    new_elem.bindings.insert("key_pressed".into(), handler);
                }
                if let Some(handler) = released {
                    new_elem.bindings.insert("key_released".into(), handler);
                }
                let new_elem = ElementRc::new(RefCell::new(new_elem));
                /*TODO: elem.bindings.insert(
                    "initial_focus".into(),
                    Expression::ElementReference(Rc::downgrade(&new_elem)).into(),
                );*/
                elem.children.push(new_elem);
            }
        }
    });
}
