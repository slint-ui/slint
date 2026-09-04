// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashSet;

use crate::diagnostics::BuildDiagnostics;
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
                    true => crate::diagnostics::DiagnosticLevel::Warning,
                    false => crate::diagnostics::DiagnosticLevel::Error,
                };
                crate::object_tree::visit_element_expressions(elem, |expr, name, _| {
                    if let Some(name) = name {
                        let lookup =
                            elem.borrow().lookup_property(name, PropertyLookupMode::InternalName);
                        if lookup.declared_pure.unwrap_or(false)
                            || lookup.property_type.is_property_type()
                        {
                            ensure_pure(expr, Some(diag), level, &mut Default::default());
                        }
                    } else {
                        // model expression must be pure
                        ensure_pure(expr, Some(diag), level, &mut Default::default());
                    };
                })
            },
        )
    }
}

/// Whether evaluating `expr` has no side effect: it assigns no property and calls only pure functions.
pub(super) fn is_pure(expr: &Expression) -> bool {
    ensure_pure(expr, None, crate::diagnostics::DiagnosticLevel::Error, &mut Default::default())
}

fn ensure_pure(
    expr: &Expression,
    mut diag: Option<&mut BuildDiagnostics>,
    level: crate::diagnostics::DiagnosticLevel,
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
            if let Some(diag) = diag.as_deref_mut() {
                diag.push_diagnostic(
                    format!("Call of impure callback '{}'", nr.declared_name()),
                    source_location,
                    level,
                );
            }
            r = false;
        }
        Expression::FunctionCall { function: Callable::Function(nr), source_location, .. } => {
            match nr
                .element()
                .borrow()
                .lookup_property(nr.name(), PropertyLookupMode::InternalName)
                .declared_pure
            {
                Some(true) => (),
                Some(false) => {
                    if let Some(diag) = diag.as_deref_mut() {
                        diag.push_diagnostic(
                            format!("Call of impure function '{}'", nr.declared_name(),),
                            source_location,
                            level,
                        );
                    }
                    r = false;
                }
                None => {
                    if recursion_test.insert(nr.clone()) {
                        let element = nr.element();
                        let element = element.borrow();
                        let body = element.binding_cell_including_synthetic(nr.name());
                        match body.map(|body| body.try_borrow()) {
                            // A native member function such as 'TextInput.cut()' has no body to inspect,
                            // so it can't be shown pure.
                            None => {
                                if let Some(diag) = diag.as_deref_mut() {
                                    diag.push_diagnostic(
                                        format!("Call of impure function '{}'", nr.declared_name()),
                                        source_location,
                                        level,
                                    );
                                }
                                r = false;
                            }
                            // The expression visitor holds a mutable borrow on the body it is visiting.
                            // Failing to borrow means the call recursed into the function under analysis,
                            // a binding loop that is reported elsewhere.
                            Some(Err(_)) => (),
                            Some(Ok(body)) => {
                                if !ensure_pure(&body.expression, None, level, recursion_test) {
                                    if let Some(diag) = diag.as_deref_mut() {
                                        diag.push_diagnostic(
                                            format!(
                                                "Call of impure function '{}'",
                                                nr.declared_name()
                                            ),
                                            source_location,
                                            level,
                                        );
                                    }
                                    r = false;
                                }
                            }
                        }
                    }
                }
            }
        }
        Expression::FunctionCall { function: Callable::Builtin(func), source_location, .. }
            if !func.is_pure() =>
        {
            if let Some(diag) = diag.as_deref_mut() {
                diag.push_diagnostic("Call of impure function".into(), source_location, level);
            }
            r = false;
        }
        Expression::SelfAssignment { node, .. } => {
            if let Some(diag) = diag.as_deref_mut() {
                diag.push_diagnostic("Assignment in a pure context".into(), node, level);
            }
            r = false;
        }
        _ => (),
    });
    r
}
