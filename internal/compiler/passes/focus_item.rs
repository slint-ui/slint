// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

//! This pass follows the forward-focus property on the root element to determine the initial focus item
//! as well as handle the forward for `focus()` calls in code.

use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Spanned};
use crate::expression_tree::{BindingExpression, BuiltinFunction, Expression};
use crate::langtype::ElementType;
use crate::object_tree::*;
use by_address::ByAddress;
use std::collections::HashSet;

enum FocusCheckResult {
    ElementIsFocusable,
    FocusForwarded(ElementRc, SourceLocation),
    ElementIsNotFocusable,
}

pub fn get_explicit_forward_focus(
    element: &ElementRc,
) -> Option<std::cell::Ref<std::cell::RefCell<BindingExpression>>> {
    let element = element.borrow();
    if element.bindings.contains_key("forward-focus") {
        Some(std::cell::Ref::map(element, |elem| &elem.bindings["forward-focus"]))
    } else {
        None
    }
}

fn element_focus_check(element: &ElementRc) -> FocusCheckResult {
    if let Some(forwarded_focus_binding) = get_explicit_forward_focus(element) {
        let forwarded_focus_binding = forwarded_focus_binding.borrow();
        if let Expression::ElementReference(target) = &forwarded_focus_binding.expression {
            return FocusCheckResult::FocusForwarded(
                target.upgrade().unwrap(),
                forwarded_focus_binding.to_source_location(),
            );
        } else {
            assert!(matches!(forwarded_focus_binding.expression, Expression::Invalid), "internal error: forward-focus property is of type ElementReference but received non-element-reference binding");
        }
    }

    if matches!(&element.borrow().base_type, ElementType::Builtin(b) if b.accepts_focus) {
        return FocusCheckResult::ElementIsFocusable;
    }

    FocusCheckResult::ElementIsNotFocusable
}

fn find_focusable_element(
    mut element: ElementRc,
    diag: &mut BuildDiagnostics,
) -> Option<ElementRc> {
    let mut last_focus_forward_location = None;
    let mut visited = HashSet::new();
    visited.insert(ByAddress(element.clone()));
    loop {
        match element_focus_check(&element) {
            FocusCheckResult::ElementIsFocusable => break Some(element),
            FocusCheckResult::FocusForwarded(forwarded_element, location) => {
                if Rc::ptr_eq(&element, &forwarded_element) {
                    diag.push_error("forward-focus can't refer to itself".into(), &location);
                    break Some(element);
                }
                if !visited.insert(ByAddress(forwarded_element.clone())) {
                    diag.push_error("forward-focus loop".into(), &location);
                    break Some(forwarded_element);
                }
                element = forwarded_element;
                last_focus_forward_location = Some(location);
            }
            FocusCheckResult::ElementIsNotFocusable => {
                if let Some(location) = last_focus_forward_location {
                    diag.push_error("element is not focusable".into(), &location);
                }
                break None;
            }
        }
    }
}

/// Ensure that all element references in SetFocusItem calls point to elements that can accept the focus, following
/// any `forward-focus` chains if needed.
fn resolve_element_reference_in_set_focus_call(expr: &mut Expression, diag: &mut BuildDiagnostics) {
    if let Expression::FunctionCall { function, arguments, source_location } = expr {
        if let Expression::BuiltinFunctionReference(BuiltinFunction::SetFocusItem, _) =
            function.as_ref()
        {
            if arguments.len() != 1 {
                panic!("internal compiler error: Invalid argument generated for SetFocusItem call");
            }
            if let Expression::ElementReference(weak_focus_target) = &mut arguments[0] {
                let focus_target = weak_focus_target.upgrade().expect(
                    "internal compiler error: weak SetFocusItem parameter cannot be dangling",
                );
                match find_focusable_element(focus_target, diag) {
                    Some(new_focus_target) => {
                        *weak_focus_target = Rc::downgrade(&new_focus_target);
                    }
                    None => diag.push_error(
                        "focus() can only be called on focusable elements".into(),
                        source_location,
                    ),
                }
                return;
            }
        }
    }
    expr.visit_mut(|e| resolve_element_reference_in_set_focus_call(e, diag))
}

pub fn resolve_element_reference_in_set_focus_calls(
    component: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    visit_all_expressions(component, |e, _| resolve_element_reference_in_set_focus_call(e, diag));
}

/// Generate setup code to pass window focus to the root item or a forwarded focus if applicable.
pub fn determine_initial_focus_item(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    if let Some(root_focus_item) = find_focusable_element(component.root_element.clone(), diag) {
        let setup_code = Expression::FunctionCall {
            function: Box::new(Expression::BuiltinFunctionReference(
                BuiltinFunction::SetFocusItem,
                None,
            )),
            arguments: vec![Expression::ElementReference(Rc::downgrade(&root_focus_item))],
            source_location: None,
        };

        component.init_code.borrow_mut().focus_setting_code.push(setup_code);
    }
}

/// The `forward-focus` property is not a real property that can be generated, so remove any bindings to it
/// to avoid them being materialized.
pub fn erase_forward_focus_properties(component: &Rc<Component>) {
    recurse_elem_no_borrow(&component.root_element, &(), &mut |elem, _| {
        elem.borrow_mut().bindings.remove("forward-focus");
    })
}
