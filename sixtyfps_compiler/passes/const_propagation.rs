// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

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
                    return true;
                }
            }
            false
        }
        Expression::BinaryExpression { lhs, op, rhs } => {
            let mut can_inline = simplify_expression(lhs);
            can_inline &= simplify_expression(rhs);

            let new = match (*op, &mut **lhs, &mut **rhs) {
                ('+', Expression::StringLiteral(a), Expression::StringLiteral(b)) => {
                    Some(Expression::StringLiteral(format!("{}{}", a, b)))
                }
                ('+', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, un2))
                    if un1 == un2 =>
                {
                    Some(Expression::NumberLiteral(*a + *b, *un1))
                }
                ('-', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, un2))
                    if un1 == un2 =>
                {
                    Some(Expression::NumberLiteral(*a - *b, *un1))
                }
                // TODO: * and / are more complicated because they need to take care about units
                ('=' | '!', Expression::NumberLiteral(a, _), Expression::NumberLiteral(b, _)) => {
                    Some(Expression::BoolLiteral((a == b) == (*op == '=')))
                }
                ('=' | '!', Expression::StringLiteral(a), Expression::StringLiteral(b)) => {
                    Some(Expression::BoolLiteral((a == b) == (*op == '=')))
                }
                ('=' | '!', Expression::EnumerationValue(a), Expression::EnumerationValue(b)) => {
                    Some(Expression::BoolLiteral((a == b) == (*op == '=')))
                }
                // TODO: more types and more comparison operators
                ('&', Expression::BoolLiteral(false), _) => {
                    can_inline = true;
                    Some(Expression::BoolLiteral(false))
                }
                ('&', _, Expression::BoolLiteral(false)) => {
                    can_inline = true;
                    Some(Expression::BoolLiteral(false))
                }
                ('&', Expression::BoolLiteral(true), e) => Some(std::mem::take(e)),
                ('&', e, Expression::BoolLiteral(true)) => Some(std::mem::take(e)),
                ('|', Expression::BoolLiteral(true), _) => {
                    can_inline = true;
                    Some(Expression::BoolLiteral(true))
                }
                ('|', _, Expression::BoolLiteral(true)) => {
                    can_inline = true;
                    Some(Expression::BoolLiteral(true))
                }
                ('|', Expression::BoolLiteral(false), e) => Some(std::mem::take(e)),
                ('|', e, Expression::BoolLiteral(false)) => Some(std::mem::take(e)),
                _ => None,
            };
            if let Some(new) = new {
                *expr = new;
            }
            can_inline
        }
        Expression::Cast { from, to } => {
            let can_inline = simplify_expression(from);
            let new = if from.ty() == *to {
                Some(std::mem::take(&mut **from))
            } else {
                match (&**from, to) {
                    (Expression::NumberLiteral(x, Unit::None), Type::String) => {
                        Some(Expression::StringLiteral((*x).to_string()))
                    }
                    _ => None,
                }
            };
            if let Some(new) = new {
                *expr = new;
            }
            can_inline
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
        if let Some(decl) = element.clone().borrow().property_declarations.get(nr.name()) {
            if let Some(alias) = &decl.is_alias {
                return extract_constant_property_reference(alias);
            }
        } else if let Type::Component(c) = &element.clone().borrow().base_type {
            element = c.root_element.clone();
            continue;
        }

        // There is no binding for this property, return the default value
        return Some(Expression::default_value_for_type(&nr.ty()));
    };
    if !(simplify_expression(&mut expression)) {
        return None;
    }
    Some(expression)
}
