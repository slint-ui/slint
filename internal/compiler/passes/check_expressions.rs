// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, Expression};
use crate::object_tree::{visit_all_expressions, Component};
use crate::parser::SyntaxKind;

/// Check the validity of expressions
///
/// - Make sure that there is no uncalled member function or macro
pub fn check_expressions(doc: &crate::object_tree::Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        visit_all_expressions(component, |e, _| check_expression(component, e, diag));
    }
}

fn check_expression(component: &Rc<Component>, e: &Expression, diag: &mut BuildDiagnostics) {
    match e {
        Expression::MemberFunction { base_node, .. } => {
            if base_node.as_ref().is_some_and(|n| n.kind() == SyntaxKind::QualifiedName) {
                // Must already have been be reported in Expression::from_expression_node
                debug_assert!(diag.has_errors());
            } else {
                diag.push_error("Member function must be called".into(), base_node);
            }
        }
        Expression::BuiltinMacroReference(_, node) => {
            diag.push_error("Builtin function must be called".into(), node);
        }
        Expression::BuiltinFunctionReference(BuiltinFunction::GetWindowScaleFactor, loc) => {
            if component.is_global() {
                diag.push_error("Cannot convert between logical and physical length in a global component, because the scale factor is not known".into(), loc);
            }
        }
        Expression::BuiltinFunctionReference(BuiltinFunction::GetWindowDefaultFontSize, loc) => {
            if component.is_global() {
                diag.push_error("Cannot convert between rem and logical length in a global component, because the default font size is not known".into(), loc);
            }
        }
        _ => e.visit(|e| check_expression(component, e, diag)),
    }
}
