/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! This pass follows the initial_focus property on the root element to determine the initial focus item

use std::rc::Rc;

use crate::{
    diagnostics::BuildDiagnostics,
    expression_tree::{BuiltinFunction, Expression},
    object_tree::*,
};

pub fn determine_initial_focus_item(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    let mut focus_item_candidate = component.root_element.clone();
    if !focus_item_candidate.borrow().bindings.contains_key("initial_focus") {
        return;
    }

    let focus_item = loop {
        let (focus_target, binding_location) = {
            let mut focus_item_mut = focus_item_candidate.borrow_mut();
            let binding = focus_item_mut.bindings.remove("initial_focus").unwrap();
            focus_item_mut.property_declarations.remove("initial_focus");
            if let Expression::ElementReference(target) = &binding.expression {
                (target.upgrade().unwrap(), binding.clone())
            } else {
                diag.push_error(
                    "internal error: initial_focus property is of type ElementReference but received non-element-reference binding".to_owned(),
                    &binding,
                );
                break None;
            }
        };

        if focus_target.borrow().bindings.contains_key("initial_focus") {
            focus_item_candidate = focus_target;
        } else {
            if let Some(native_class) = focus_target.borrow().native_class() {
                if native_class.lookup_property("has_focus").is_some() {
                    break Some(focus_target.clone());
                } else {
                    diag.push_error("element is not focusable".to_owned(), &binding_location);
                }
            } else {
                diag.push_error("internal error: item targeted by initial_focus does not have an underlying native class".to_owned(), &binding_location);
            }
            break None;
        }
    };

    if let Some(focus_item) = focus_item {
        let setup_code = Expression::FunctionCall {
            function: Box::new(Expression::BuiltinFunctionReference(BuiltinFunction::SetFocusItem)),
            arguments: vec![Expression::ElementReference(Rc::downgrade(&focus_item))],
        };

        component.setup_code.borrow_mut().push(setup_code);
    }

    // Remove any stray bindings in other inlined elements to avoid materializing them.
    recurse_elem(&component.root_element, &(), &mut |element_rc, _| {
        element_rc.borrow_mut().bindings.remove("initial_focus");
    });
}
