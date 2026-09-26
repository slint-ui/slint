// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Resolves `Path`'s `stroke-dash-array` at compile time.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Expression, Unit};
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::SmolStr;
use std::rc::Rc;

pub fn compile_dash_array(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem_, _| {
        if elem_.borrow().builtin_type().is_none_or(|bt| bt.name != "Path") {
            return;
        }

        let Some(binding) = elem_.borrow_mut().take_binding("stroke-dash-array") else { return };

        let Expression::StringLiteral(text) = binding.expression.ignore_debug_hooks() else {
            diag.push_error(
                "`stroke-dash-array` must be a string literal: it is resolved at compile time"
                    .into(),
                &binding,
            );
            return;
        };

        let dash_array = parse_dash_array(text);
        let expr = Expression::Cast {
            from: Box::new(Expression::Array {
                element_ty: Type::Float32,
                values: dash_array
                    .into_iter()
                    .map(|v| Expression::NumberLiteral(v as _, Unit::None))
                    .collect(),
            }),
            to: Type::DashArray,
        };
        elem_.borrow_mut().set_binding(SmolStr::new_static("stroke-dash-array"), expr.into());
    });
}

/// Whitespace separated non-negative numbers.
/// An empty list, a negative or a non-numeric entry yields an empty pattern (solid outline).
/// An odd number of entries is repeated to make the list even.
fn parse_dash_array(text: &str) -> Vec<f32> {
    let mut dash_array = Vec::new();
    for token in text.split_whitespace() {
        match token.parse::<f32>() {
            Ok(v) if v >= 0.0 => dash_array.push(v),
            _ => return Vec::new(),
        }
    }
    if dash_array.len() % 2 == 1 {
        dash_array = dash_array.repeat(2);
    }
    dash_array
}
