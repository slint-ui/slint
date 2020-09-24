/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Pass that create a state property, and change all the binding to depend on that property

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::*;
use crate::object_tree::*;
use crate::typeregister::Type;
use std::rc::Rc;

pub fn lower_states(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    recurse_elem(&component.root_element, &(), &mut |elem, _| lower_state_in_element(elem, diag));
}

fn lower_state_in_element(root_element: &ElementRc, _diag: &mut BuildDiagnostics) {
    if root_element.borrow().states.is_empty() {
        return;
    }
    let state_property = compute_state_property_name(root_element);
    let state_property_ref = Expression::PropertyReference(NamedReference {
        element: Rc::downgrade(root_element),
        name: state_property.clone(),
    });
    let mut state_value = Expression::NumberLiteral(0., Unit::None);
    let states = std::mem::take(&mut root_element.borrow_mut().states);
    for (idx, state) in states.into_iter().enumerate().rev() {
        if let Some(condition) = &state.condition {
            state_value = Expression::Condition {
                condition: Box::new(condition.clone()),
                true_expr: Box::new(Expression::NumberLiteral((idx + 1) as _, Unit::None)),
                false_expr: Box::new(std::mem::take(&mut state_value)),
            };
        }
        for (ne, expr) in state.property_changes {
            let e = ne.element.upgrade().unwrap();
            let property_expr = expression_for_property(&e, ne.name.as_str());
            e.borrow_mut().bindings.insert(
                ne.name,
                Expression::Condition {
                    condition: Box::new(Expression::BinaryExpression {
                        lhs: Box::new(state_property_ref.clone()),
                        rhs: Box::new(Expression::NumberLiteral((idx + 1) as _, Unit::None)),
                        op: '=',
                    }),
                    true_expr: Box::new(expr),
                    false_expr: Box::new(property_expr),
                }
                .into(),
            );
        }
    }
    root_element.borrow_mut().property_declarations.insert(
        state_property.clone(),
        PropertyDeclaration { property_type: Type::Int32, ..PropertyDeclaration::default() },
    );
    root_element.borrow_mut().bindings.insert(state_property.clone(), state_value.into());
}

/// Returns a suitable unique name for the "state" property
fn compute_state_property_name(root_element: &ElementRc) -> String {
    let mut property_name = "state".to_owned();
    while root_element.borrow().lookup_property(property_name.as_ref()) != Type::Invalid {
        property_name += "_";
    }
    property_name
}

/// Return the expression binding currently associated to the given property
fn expression_for_property(element: &ElementRc, name: &str) -> Expression {
    let mut element_it = Some(element.clone());
    while let Some(element) = element_it {
        if let Some(e) = element.borrow().bindings.get(name) {
            return e.expression.clone();
        }
        element_it = if let Type::Component(base) = &element.borrow().base_type {
            Some(base.root_element.clone())
        } else {
            None
        };
    }
    Expression::default_value_for_type(&element.borrow().lookup_property(name))
}
