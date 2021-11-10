/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Try to simplify property bindings by propagating constant expressions

use crate::expression_tree::*;
use crate::langtype::Type;
use crate::object_tree::*;

pub fn const_propagation(component: &Component) {
    visit_all_expressions(component, |expr, ty| {
        if matches!(ty(), Type::Callback { .. }) {
            return;
        }
        simplify_expression(expr);
    });
}

/// Returns false if the expression still contains a reference to an element
fn simplify_expression(expr: &mut Expression) -> bool {
    match expr {
        Expression::PropertyReference(nr) => {
            if nr.is_constant() {
                // Inline the constant value
                if let Some(result) = extract_constant_property_reference(nr) {
                    *expr = result;
                } else {
                    return false;
                }
            }
            true
        }
        Expression::CallbackReference { .. } => false,
        Expression::ElementReference { .. } => false,
        // FIXME
        Expression::LayoutCacheAccess { .. } => false,
        Expression::SolveLayout { .. } => false,
        Expression::ComputeLayoutInfo { .. } => false,
        _ => {
            let mut result = true;
            expr.visit_mut(|expr| result &= simplify_expression(expr));
            result
        }
    }
}

/// Will extract the property binding from the given named reference
/// and propagate constant expression within it. If that's possible,
/// return the new expression
fn extract_constant_property_reference(nr: &NamedReference) -> Option<Expression> {
    debug_assert!(nr.is_constant());
    // find the binding.
    let mut element = nr.element();
    let mut expression = loop {
        if let Some(binding) = element.borrow().bindings.get(nr.name()) {
            let binding = binding.borrow();
            if !binding.two_way_bindings.is_empty() {
                // TODO: In practice, we should still find out what the real binding is
                // and solve that.
                return None;
            }
            if !matches!(binding.expression, Expression::Invalid) {
                break binding.expression.clone();
            }
        };
        if let Type::Component(c) = &element.clone().borrow().base_type {
            if !element.borrow().property_declarations.contains_key(nr.name()) {
                element = c.root_element.clone();
                continue;
            }
        }
        // There is no binding for this property, return the default value
        return Some(Expression::default_value_for_type(&nr.ty()));
    };
    if !(simplify_expression(&mut expression)) {
        return None;
    }
    Some(expression)
}
