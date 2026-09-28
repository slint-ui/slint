// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{BindingExpression, ElementRc, ElementType, PropertyDeclaration};
use crate::expression_tree::{Callable, Expression, TwoWayBinding};
use crate::langtype::Type;
use crate::namedreference::NamedReference;
use crate::symbol_counters::SymbolCounters;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

#[derive(Default)]
pub(crate) struct ForwardedReferenceCache {
    forwarded_references: HashMap<NamedReference, NamedReference>,
}

pub(crate) enum InheritedExpression {
    Expression(Expression),
    /// A two-way binding in the base binds the property. Carries the base root it is on, so a
    /// caller that wants the value can resolve the link itself with [`follow_two_way_bindings`]
    /// and rebase the result with [`rebase_expression_to_instance`].
    TwoWayBinding(ElementRc),
    Unbound,
}

/// The property whose binding holds the value of `property_name`, reached by following the
/// two-way bindings that [`remove_aliases`](crate::passes::remove_aliases) merges into a single
/// property. Following again from the result returns it unchanged.
///
/// Returns `None` for a link that stays two separate properties: to a global, to a struct field,
/// or to model data. This models a subset of `PropertySets::add_link`, which decides the merge
/// for real and can only do so once every component is inlined; where the two disagree, a caller
/// gets the conservative answer. `None` for a cycle as well, which `add_link` reports.
pub(crate) fn follow_two_way_bindings(
    element: &ElementRc,
    property_name: &str,
) -> Option<NamedReference> {
    let mut current = NamedReference::new(element, property_name.into());
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(current.clone()) {
            return None;
        }
        let current_element = current.element();
        let next = {
            let current_element = current_element.borrow();
            let Some(binding) = current_element.binding(current.name()) else { break };
            // A property that also has an expression of its own is where the chain ends: that
            // expression is the value the merged property takes.
            if !matches!(binding.expression, Expression::Invalid) {
                break;
            }
            match binding.two_way_bindings.first() {
                Some(TwoWayBinding::Property { property, field_access })
                    if field_access.is_empty()
                        && Weak::ptr_eq(
                            &property.element().borrow().enclosing_component,
                            &current_element.enclosing_component,
                        ) =>
                {
                    property.clone()
                }
                Some(_) => return None,
                None => break,
            }
        };
        current = next;
    }
    Some(current)
}

/// The expression a base component binds `property_name` to, rebased onto `element`.
pub(crate) fn forward_inherited_expression(
    element: &ElementRc,
    property_name: &str,
    symbol_counters: &SymbolCounters,
    forwarded_references: &mut ForwardedReferenceCache,
) -> InheritedExpression {
    let ElementType::Component(base_component) = &element.borrow().base_type else {
        return InheritedExpression::Unbound;
    };
    let mut current_base_root = base_component.root_element.clone();

    loop {
        let binding = current_base_root
            .borrow()
            .binding(property_name)
            .map(|binding| (!binding.two_way_bindings.is_empty(), binding.expression.clone()));
        if let Some((is_two_way_binding, mut expression)) = binding {
            if is_two_way_binding {
                return InheritedExpression::TwoWayBinding(current_base_root.clone());
            }
            if !matches!(expression, Expression::Invalid) {
                rebase_expression_to_instance(
                    &mut expression,
                    &current_base_root,
                    element,
                    symbol_counters,
                    forwarded_references,
                );
                return InheritedExpression::Expression(expression);
            }
        }

        let next_source_root = {
            let source_root = current_base_root.borrow();
            let ElementType::Component(base_component) = &source_root.base_type else {
                return InheritedExpression::Unbound;
            };
            base_component.root_element.clone()
        };
        current_base_root = next_source_root;
    }
}

pub(crate) fn rebase_expression_to_instance(
    expression: &mut Expression,
    base_root_element: &ElementRc,
    target_instance: &ElementRc,
    symbol_counters: &SymbolCounters,
    forwarded_references: &mut ForwardedReferenceCache,
) {
    expression.visit_recursive_mut(&mut |expression| match expression {
        Expression::PropertyReference(named_reference)
        | Expression::FunctionCall {
            function: Callable::Callback(named_reference) | Callable::Function(named_reference),
            ..
        } => {
            let referenced_element = named_reference.element();
            if Rc::ptr_eq(&referenced_element, base_root_element) {
                *named_reference =
                    NamedReference::new(target_instance, named_reference.name().clone());
            } else if Weak::ptr_eq(
                &referenced_element.borrow().enclosing_component,
                &base_root_element.borrow().enclosing_component,
            ) {
                let forwarded_reference = forward_reference(
                    named_reference,
                    base_root_element,
                    symbol_counters,
                    forwarded_references,
                );
                *named_reference =
                    NamedReference::new(target_instance, forwarded_reference.name().clone());
            }
        }
        _ => (),
    });
}

fn forward_reference(
    original: &NamedReference,
    base_root: &ElementRc,
    symbol_counters: &SymbolCounters,
    forwarded_references: &mut ForwardedReferenceCache,
) -> NamedReference {
    if let Some(existing) = forwarded_references.forwarded_references.get(original) {
        return existing.clone();
    }

    let property_type = original.ty();
    let property_name = symbol_counters.generate_name("forward_reference_");
    let binding = match &property_type {
        Type::Callback(function) | Type::Function(function) => {
            let arguments = function
                .args
                .iter()
                .enumerate()
                .map(|(index, argument_type)| Expression::FunctionParameterReference {
                    index,
                    ty: argument_type.clone(),
                })
                .collect();
            let function = if matches!(property_type, Type::Callback(_)) {
                Callable::Callback(original.clone())
            } else {
                Callable::Function(original.clone())
            };
            Expression::FunctionCall { function, arguments, source_location: None }
        }
        _ => Expression::PropertyReference(original.clone()),
    };

    base_root.borrow_mut().property_declarations.insert(
        property_name.clone(),
        PropertyDeclaration { property_type, ..PropertyDeclaration::default() },
    );
    base_root.borrow_mut().set_binding(property_name.clone(), BindingExpression::from(binding));

    let forwarded_reference = NamedReference::new(base_root, property_name);
    forwarded_references.forwarded_references.insert(original.clone(), forwarded_reference.clone());
    forwarded_reference
}
