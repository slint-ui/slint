// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass lowers the access to the global TextInputInterface.text-input-focused to getter or setter.

use crate::builtin_functions::BuiltinFunction;
use crate::expression_tree::Expression;
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, visit_all_expressions};
use std::rc::Rc;

pub fn lower_text_input_interface(component: &Rc<Component>) {
    visit_all_expressions(component, |e, _| {
        e.visit_recursive_mut(&mut |e| match e {
            Expression::PropertyReference(nr) => match prop_name(nr) {
                Some("text-input-focused") => {
                    *e = Expression::FunctionCall {
                        function: BuiltinFunction::TextInputFocused.into(),
                        arguments: Vec::new(),
                        source_location: None,
                    };
                }
                Some("text-input-type") => {
                    *e = Expression::FunctionCall {
                        function: BuiltinFunction::TextInputType.into(),
                        arguments: Vec::new(),
                        source_location: None,
                    };
                }
                Some("text-input-method-hints") => {
                    *e = Expression::FunctionCall {
                        function: BuiltinFunction::TextInputMethodHints.into(),
                        arguments: Vec::new(),
                        source_location: None,
                    };
                }
                _ => {}
            },
            Expression::SelfAssignment { lhs, rhs, .. } => {
                if let Expression::PropertyReference(nr) = &**lhs
                    && let Some("text-input-focused") = prop_name(nr)
                {
                    let rhs = std::mem::take(&mut **rhs);
                    *e = Expression::FunctionCall {
                        function: BuiltinFunction::SetTextInputFocused.into(),
                        arguments: vec![rhs],
                        source_location: None,
                    };
                }
            }
            _ => {}
        })
    })
}

fn prop_name(nr: &NamedReference) -> Option<&str> {
    if nr.element().borrow().builtin_type().is_none_or(|bt| bt.name != "TextInputInterface") {
        return None;
    }
    Some(nr.name())
}
