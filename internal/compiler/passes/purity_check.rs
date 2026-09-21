// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashSet;

use crate::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use crate::expression_tree::{Callable, Expression, NamedReference};
use crate::langtype::PropertyLookupMode;

/// Check that pure expression only call pure functions
pub fn purity_check(doc: &crate::object_tree::Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        crate::object_tree::recurse_elem_including_sub_components_no_borrow(
            component,
            &(),
            &mut |elem, &()| {
                let level = match elem.borrow().is_legacy_syntax {
                    true => DiagnosticLevel::Warning,
                    false => DiagnosticLevel::Error,
                };
                crate::object_tree::visit_element_expressions(elem, |expr, name, _| {
                    if let Some(name) = name {
                        let lookup =
                            elem.borrow().lookup_property(name, PropertyLookupMode::InternalName);
                        if lookup.declared_pure.unwrap_or(false)
                            || lookup.property_type.is_property_type()
                        {
                            ensure_pure(expr, Some((diag, level)), &mut Default::default());
                        }
                    } else {
                        // model expression must be pure
                        ensure_pure(expr, Some((diag, level)), &mut Default::default());
                    };
                })
            },
        )
    }
}

/// Whether evaluating `expr` has no side effect: it assigns no property and calls nothing impure.
/// A `pure` declaration is taken at face value, which the legacy syntax only warns about.
pub(super) fn is_pure(expr: &Expression) -> bool {
    ensure_pure(expr, None, &mut Default::default())
}

fn ensure_pure(
    expr: &Expression,
    mut diag: Option<(&mut BuildDiagnostics, DiagnosticLevel)>,
    recursion_test: &mut HashSet<NamedReference>,
) -> bool {
    let mut r = true;
    expr.visit_recursive(&mut |e| match e {
        Expression::FunctionCall { function: Callable::Callback(nr), source_location, .. }
            if !nr
                .element()
                .borrow()
                .lookup_property(nr.name(), PropertyLookupMode::InternalName)
                .declared_pure
                .unwrap_or(false) =>
        {
            if let Some((diag, level)) = diag.as_mut() {
                diag.push_diagnostic(
                    format!("Call of impure callback '{}'", nr.declared_name()),
                    source_location,
                    *level,
                );
            }
            r = false;
        }
        Expression::FunctionCall { function: Callable::Function(nr), source_location, .. }
            if !function_is_pure(nr, recursion_test) =>
        {
            if let Some((diag, level)) = diag.as_mut() {
                diag.push_diagnostic(
                    format!("Call of impure function '{}'", nr.declared_name()),
                    source_location,
                    *level,
                );
            }
            r = false;
        }
        Expression::FunctionCall { function: Callable::Builtin(func), source_location, .. }
            if !func.is_pure() =>
        {
            if let Some((diag, level)) = diag.as_mut() {
                diag.push_diagnostic("Call of impure function".into(), source_location, *level);
            }
            r = false;
        }
        Expression::SelfAssignment { node, .. } => {
            if let Some((diag, level)) = diag.as_mut() {
                diag.push_diagnostic("Assignment in a pure context".into(), node, *level);
            }
            r = false;
        }
        _ => (),
    });
    r
}

/// Whether calling the function `nr` is pure.
/// A private function carries no declaration, so it is judged by its body.
fn function_is_pure(nr: &NamedReference, recursion_test: &mut HashSet<NamedReference>) -> bool {
    let element = nr.element();
    let element = element.borrow();
    if let Some(declared) =
        element.lookup_property(nr.name(), PropertyLookupMode::InternalName).declared_pure
    {
        return declared;
    }
    // A function already under inspection is a cycle, reported as a binding loop elsewhere.
    if !recursion_test.insert(nr.clone()) {
        return true;
    }
    match element.binding_cell_including_synthetic(nr.name()).map(|body| body.try_borrow()) {
        Some(Ok(body)) => ensure_pure(&body.expression, None, recursion_test),
        // The expression visitor holds a mutable borrow on the body it is visiting, and that
        // function isn't in `recursion_test`. A failed borrow is a call back into it: a cycle too.
        Some(Err(_)) => true,
        // Only reached for a lookup that already failed with an error.
        None => true,
    }
}
