/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::Expression;
use crate::object_tree::{recurse_elem, visit_element_expressions};

/// Check the validity of expressions
///
/// - Make sure that there is no uncalled member function or macro
pub fn check_expressions(doc: &crate::object_tree::Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        recurse_elem(&component.root_element, &(), &mut |elem, _| {
            visit_element_expressions(elem, |e, _, _| check_expression(e, diag));
        })
    }
}

fn check_expression(e: &Expression, diag: &mut BuildDiagnostics) -> () {
    match e {
        Expression::MemberFunction { base_node, .. } => {
            diag.push_error("Member function must be called".into(), base_node);
        }
        Expression::BuiltinMacroReference(_, node) => {
            diag.push_error("Builtin function must be called".into(), node);
        }
        _ => e.visit(|e| check_expression(e, diag)),
    }
}
