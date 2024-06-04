// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Passe lower the access to the global TextInputInterface.text-input-focused to getter or setter.

use crate::expression_tree::{BuiltinFunction, Expression};
use crate::namedreference::NamedReference;
use crate::object_tree::{visit_all_expressions, Component};
use std::rc::Rc;

pub fn lower_text_input_interface(component: &Rc<Component>) {
    visit_all_expressions(component, |e, _| {
        e.visit_recursive_mut(&mut |e| match e {
            Expression::PropertyReference(nr) if is_input_text_focused_prop(nr) => {
                *e = Expression::FunctionCall {
                    function: Expression::BuiltinFunctionReference(
                        BuiltinFunction::TextInputFocused,
                        None,
                    )
                    .into(),
                    arguments: vec![],
                    source_location: None,
                };
            }
            Expression::SelfAssignment{ lhs, rhs, .. } => {
                if matches!(&**lhs, Expression::PropertyReference(nr)  if is_input_text_focused_prop(nr) ) {
                    let rhs = std::mem::take(&mut **rhs);
                    *e = Expression::FunctionCall {
                        function: Expression::BuiltinFunctionReference(
                            BuiltinFunction::SetTextInputFocused,
                            None,
                        )
                        .into(),
                        arguments: vec![rhs],
                        source_location: None,
                    };
                }

            }
            _ => {}
        })
    })
}

fn is_input_text_focused_prop(nr: &NamedReference) -> bool {
    if !nr.element().borrow().builtin_type().map_or(false, |bt| bt.name == "TextInputInterface") {
        return false;
    }
    assert_eq!(nr.name(), "text-input-focused");
    true
}
