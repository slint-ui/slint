// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::Expression;

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
                        let lookup = elem.borrow().lookup_property(name);
                        if lookup.declared_pure.unwrap_or(false)
                            || lookup.property_type.is_property_type()
                        {
                            ensure_pure(expr, Some(diag), level);
                        }
                    } else {
                        // model expression must be pure
                        ensure_pure(expr, Some(diag), level);
                    };
                })
            },
        )
    }
}

fn ensure_pure(
    expr: &Expression,
    mut diag: Option<&mut BuildDiagnostics>,
    level: crate::diagnostics::DiagnosticLevel,
) -> bool {
    let mut r = true;
    expr.visit_recursive(&mut |e| match e {
        Expression::CallbackReference(nr, node) => {
            if !nr.element().borrow().lookup_property(nr.name()).declared_pure.unwrap_or(false) {
                if let Some(diag) = diag.as_deref_mut() {
                    diag.push_diagnostic(
                        format!("Call of impure callback '{}'", nr.name()),
                        node,
                        level,
                    );
                }
                r = false;
            }
        }
        Expression::FunctionReference(nr, node) => {
            match nr.element().borrow().lookup_property(nr.name()).declared_pure {
                Some(true) => return,
                Some(false) => {
                    if let Some(diag) = diag.as_deref_mut() {
                        diag.push_diagnostic(
                            format!("Call of impure function '{}'", nr.name(),),
                            node,
                            level,
                        );
                    }
                    r = false;
                }
                None => {
                    if !ensure_pure(
                        &nr.element()
                            .borrow()
                            .bindings
                            .get(nr.name())
                            .expect("private function must be local and defined")
                            .borrow()
                            .expression,
                        None,
                        level,
                    ) {
                        if let Some(diag) = diag.as_deref_mut() {
                            diag.push_diagnostic(
                                format!("Call of impure function '{}'", nr.name()),
                                node,
                                level,
                            );
                        }
                        r = false;
                    }
                }
            }
        }
        Expression::BuiltinFunctionReference(func, node) => {
            if !func.is_pure() {
                if let Some(diag) = diag.as_deref_mut() {
                    diag.push_diagnostic("Call of impure function".into(), node, level);
                }
                r = false;
            }
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
