// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that create a state property, and change all the binding to depend on that property

use crate::diagnostics::BuildDiagnostics;
use crate::diagnostics::SourceLocation;
use crate::diagnostics::Spanned;
use crate::expression_tree::*;
use crate::langtype::ElementType;
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

pub fn lower_states(
    component: &Rc<Component>,
    tr: &crate::typeregister::TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let state_info_type = tr.lookup("StateInfo");
    assert!(matches!(state_info_type, Type::Struct(ref s) if s.name.is_some()));
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        lower_state_in_element(elem, &state_info_type, diag)
    });
}

fn lower_state_in_element(
    root_element: &ElementRc,
    state_info_type: &Type,
    diag: &mut BuildDiagnostics,
) {
    if root_element.borrow().states.is_empty() {
        return;
    }
    let has_transitions = !root_element.borrow().transitions.is_empty();
    let state_property_name = compute_state_property_name(root_element);
    let state_property = Expression::PropertyReference(NamedReference::new(
        root_element,
        state_property_name.clone(),
    ));
    let state_property_ref = if has_transitions {
        Expression::StructFieldAccess {
            base: Box::new(state_property.clone()),
            name: "current-state".into(),
        }
    } else {
        state_property.clone()
    };
    let mut affected_properties = HashSet::new();
    // Maps State name string -> integer id
    let mut states_id = HashMap::new();
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
        for (ne, expr, node) in state.property_changes {
            affected_properties.insert(ne.clone());
            let e = ne.element();
            let property_expr = match expression_for_property(&e, ne.name()) {
                ExpressionForProperty::TwoWayBinding => {
                    diag.push_error(
                    format!("Cannot change the property '{}' in a state because it is initialized with a two-way binding", ne.name()),
                    &node
                );
                    continue;
                }
                ExpressionForProperty::Expression(e) => e,
                ExpressionForProperty::InvalidBecauseOfIssue1461 => {
                    diag.push_error(
                        format!("Internal error: The expression for the default state currently cannot be represented: https://github.com/slint-ui/slint/issues/1461\nAs a workaround, add a binding for property {}", ne.name()),
                        &node
                    );
                    continue;
                }
            };
            let new_expr = Expression::Condition {
                condition: Box::new(Expression::BinaryExpression {
                    lhs: Box::new(state_property_ref.clone()),
                    rhs: Box::new(Expression::NumberLiteral((idx + 1) as _, Unit::None)),
                    op: '=',
                }),
                true_expr: Box::new(expr),
                false_expr: Box::new(property_expr),
            };
            match e.borrow_mut().bindings.entry(ne.name().clone()) {
                std::collections::btree_map::Entry::Occupied(mut e) => {
                    e.get_mut().get_mut().expression = new_expr
                }
                std::collections::btree_map::Entry::Vacant(e) => {
                    let mut r = BindingExpression::from(new_expr);
                    r.priority = 1;
                    e.insert(r.into());
                }
            };
        }
        states_id.insert(state.id, idx as i32 + 1);
    }

    root_element.borrow_mut().property_declarations.insert(
        state_property_name.clone(),
        PropertyDeclaration {
            property_type: if has_transitions { state_info_type.clone() } else { Type::Int32 },
            ..PropertyDeclaration::default()
        },
    );
    root_element
        .borrow_mut()
        .bindings
        .insert(state_property_name, RefCell::new(state_value.into()));

    lower_transitions_in_element(
        root_element,
        state_property,
        states_id,
        affected_properties,
        diag,
    );
}

fn lower_transitions_in_element(
    elem: &ElementRc,
    state_property: Expression,
    states_id: HashMap<SmolStr, i32>,
    affected_properties: HashSet<NamedReference>,
    diag: &mut BuildDiagnostics,
) {
    let transitions = std::mem::take(&mut elem.borrow_mut().transitions);
    let mut props =
        HashMap::<NamedReference, (SourceLocation, Vec<TransitionPropertyAnimation>)>::new();
    for transition in transitions {
        let state = states_id.get(&transition.state_id).unwrap_or_else(|| {
            diag.push_error(
                format!("State '{}' does not exist", transition.state_id),
                transition
                    .node
                    .DeclaredIdentifier()
                    .as_ref()
                    .map(|x| x as &dyn Spanned)
                    .unwrap_or(&transition.node as &dyn Spanned),
            );
            &0
        });

        for (p, span, animation) in transition.property_animations {
            if !affected_properties.contains(&p) {
                diag.push_error(
                    "The property is not changed as part of this transition".into(),
                    &span,
                );
                continue;
            }

            let t = TransitionPropertyAnimation {
                state_id: *state,
                is_out: transition.is_out,
                animation,
            };
            props.entry(p).or_insert_with(|| (span.clone(), vec![])).1.push(t);
        }
    }
    for (ne, (span, animations)) in props {
        let e = ne.element();
        // We check earlier that the property is in the set of changed properties, so a binding bust have been assigned
        let old_anim = e.borrow().bindings.get(ne.name()).unwrap().borrow_mut().animation.replace(
            PropertyAnimation::Transition { state_ref: state_property.clone(), animations },
        );
        if old_anim.is_some() {
            diag.push_error(
                format!(
                    "The property '{}' cannot have transition because it already has an animation",
                    ne.name()
                ),
                &span,
            );
        }
    }
}

/// Returns a suitable unique name for the "state" property
fn compute_state_property_name(root_element: &ElementRc) -> SmolStr {
    let mut property_name = "state".to_owned();
    while root_element.borrow().lookup_property(property_name.as_ref()).property_type
        != Type::Invalid
    {
        property_name += "-";
    }
    property_name.into()
}

enum ExpressionForProperty {
    TwoWayBinding,
    Expression(Expression),
    /// Workaround: the expression can't be represented with the current data structure, so make it an error for now.
    InvalidBecauseOfIssue1461,
}

/// Return the expression binding currently associated to the given property
fn expression_for_property(element: &ElementRc, name: &str) -> ExpressionForProperty {
    let mut element_it = Some(element.clone());
    let mut in_base = false;
    while let Some(elem) = element_it {
        if let Some(e) = elem.borrow().bindings.get(name) {
            let e = e.borrow();
            if !e.two_way_bindings.is_empty() {
                return ExpressionForProperty::TwoWayBinding;
            }
            let mut expr = e.expression.clone();
            if !matches!(expr, Expression::Invalid) {
                if in_base {
                    // Check that the expression is valid in the new scope
                    let mut has_invalid = false;
                    expr.visit_recursive_mut(&mut |ex| match ex {
                        Expression::PropertyReference(nr)
                        | Expression::FunctionCall {
                            function: Callable::Callback(nr) | Callable::Function(nr),
                            ..
                        } => {
                            let e = nr.element();
                            if Rc::ptr_eq(&e, &elem) {
                                *nr = NamedReference::new(element, nr.name().clone());
                            } else if Weak::ptr_eq(
                                &e.borrow().enclosing_component,
                                &elem.borrow().enclosing_component,
                            ) {
                                has_invalid = true;
                            }
                        }
                        _ => (),
                    });
                    if has_invalid {
                        return ExpressionForProperty::InvalidBecauseOfIssue1461;
                    }
                }

                return ExpressionForProperty::Expression(expr);
            }
        }
        element_it = if let ElementType::Component(base) = &elem.borrow().base_type {
            in_base = true;
            Some(base.root_element.clone())
        } else {
            None
        };
    }
    let expr = super::materialize_fake_properties::initialize(element, name).unwrap_or_else(|| {
        Expression::default_value_for_type(&element.borrow().lookup_property(name).property_type)
    });

    ExpressionForProperty::Expression(expr)
}
