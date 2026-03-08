// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, Callable, Expression};
use crate::object_tree::{Component, visit_all_expressions};

/// Check the validity of expressions
///
/// - Check that the GetWindowScaleFactor and GetWindowDefaultFontSize are not called in a global
pub fn check_expressions(doc: &crate::object_tree::Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        visit_all_expressions(component, |e, _| check_expression(component, e, diag));
    }
}

fn check_expression(component: &Rc<Component>, e: &Expression, diag: &mut BuildDiagnostics) {
    match e {
        Expression::FunctionCall { function: Callable::Builtin(b), source_location, .. } => {
            match b {
                BuiltinFunction::GetWindowScaleFactor => {
                    if component.is_global() {
                        diag.push_error("Cannot convert between logical and physical length in a global component, because the scale factor is not known".into(), source_location);
                    }
                }
                BuiltinFunction::GetWindowDefaultFontSize => {
                    if component.is_global() {
                        diag.push_error("Cannot convert between rem and logical length in a global component, because the default font size is not known".into(), source_location);
                    }
                }
                _ => {}
            }
        }
        Expression::Unwrap { .. } | Expression::NullCoalesce { .. } => {
            // Type checking for unwrap and null-coalesce is handled by their ty() methods
            // which return Type::Invalid for invalid usage, causing type mismatches to be
            // reported elsewhere in the compiler
        }
        _ => {}
    }
    e.visit(|e| check_expression(component, e, diag))
}
