// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that create a state property, and change all the binding to depend on that property

use crate::diagnostics::BuildDiagnostics;
use crate::diagnostics::SourceLocation;
use crate::diagnostics::Spanned;
use crate::expression_tree::*;
use crate::langtype::{PropertyLookupMode, Type};
use crate::object_tree::forward_inherited_expression::{
    ForwardedReferenceCache, InheritedExpression, forward_inherited_expression,
};
use crate::object_tree::*;
use crate::symbol_counters::SymbolCounters;
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub fn lower_states(
    component: &Rc<Component>,
    symbol_counters: &SymbolCounters,
    forwarded_references: &mut ForwardedReferenceCache,
    diag: &mut BuildDiagnostics,
) {
    let state_info_type = crate::typeregister::BUILTIN.state_info_type.clone().into();
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        lower_state_in_element(elem, &state_info_type, symbol_counters, forwarded_references, diag)
    });
}

fn lower_state_in_element(
    root_element: &ElementRc,
    state_info_type: &Type,
    symbol_counters: &SymbolCounters,
    forwarded_references: &mut ForwardedReferenceCache,
    diag: &mut BuildDiagnostics,
) {
    if root_element.borrow().states.is_empty() {
        return;
    }
    let has_transitions = !root_element.borrow().transitions.is_empty();
    let state_property_nr = crate::layout::create_new_prop(
        root_element,
        SmolStr::new_static("state"),
        if has_transitions { state_info_type.clone() } else { Type::Int32 },
    );
    let state_property = Expression::PropertyReference(state_property_nr.clone());
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
        for (property_reference, expr, node) in state.property_changes {
            affected_properties.insert(property_reference.clone());
            let element = property_reference.element();
            let property_expr = match expression_for_property(
                &element,
                property_reference.name(),
                symbol_counters,
                forwarded_references,
            ) {
                ExpressionForProperty::TwoWayBinding => {
                    diag.push_error(
                    format!("Cannot change the property '{}' in a state because it is initialized with a two-way binding", property_reference.name()),
                    &node
                );
                    continue;
                }
                ExpressionForProperty::Expression(e) => e,
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

            let name = property_reference.name();
            if let Some(cell) = element.borrow().binding_cell_including_synthetic(name) {
                // A synthetic hook is upgraded in place; a real binding's hook survives inside
                // `property_expr` (the false-branch of `new_expr`), so replacing it is correct.
                cell.borrow_mut().set_value_expression(new_expr);
            } else {
                let mut r = BindingExpression::from(new_expr);
                r.priority = 1;
                element.borrow_mut().set_binding(name.clone(), r);
            }
        }
        states_id.insert(state.id, idx as i32 + 1);
    }

    root_element.borrow_mut().set_binding(state_property_nr.name().clone(), state_value.into());

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
                direction: transition.direction,
                animation,
            };
            props.entry(p).or_insert_with(|| (span.clone(), Vec::new())).1.push(t);
        }
    }
    for (ne, (span, animations)) in props {
        let e = ne.element();
        // We check earlier that the property is in the set of changed properties, so a binding bust have been assigned
        let old_anim = e.borrow().binding_mut(ne.name()).unwrap().animation.replace(
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

enum ExpressionForProperty {
    TwoWayBinding,
    Expression(Expression),
}

/// Return the expression binding currently associated to the given property
fn expression_for_property(
    element: &ElementRc,
    name: &str,
    symbol_counters: &SymbolCounters,
    forwarded_references: &mut ForwardedReferenceCache,
) -> ExpressionForProperty {
    let local_binding = element
        .borrow()
        .binding(name)
        .map(|binding| (!binding.two_way_bindings.is_empty(), binding.expression.clone()));
    if let Some((is_two_way_binding, expression)) = local_binding {
        if is_two_way_binding {
            return ExpressionForProperty::TwoWayBinding;
        }
        if !matches!(expression, Expression::Invalid) {
            return ExpressionForProperty::Expression(expression);
        }
    }

    match forward_inherited_expression(element, name, symbol_counters, forwarded_references) {
        InheritedExpression::Expression(expression) => {
            return ExpressionForProperty::Expression(expression);
        }
        InheritedExpression::TwoWayBinding => return ExpressionForProperty::TwoWayBinding,
        InheritedExpression::Unbound => {}
    }

    let expression =
        super::materialize_fake_properties::initialize(element, name).unwrap_or_else(|| {
            Expression::default_value_for_type(
                &element
                    .borrow()
                    .lookup_property(name, PropertyLookupMode::InternalName)
                    .property_type,
            )
        });

    ExpressionForProperty::Expression(expression)
}
