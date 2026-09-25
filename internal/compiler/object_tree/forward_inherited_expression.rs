// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{BindingExpression, ElementRc, ElementType, PropertyDeclaration};
use crate::expression_tree::{Callable, Expression};
use crate::langtype::Type;
use crate::namedreference::NamedReference;
use crate::symbol_counters::SymbolCounters;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

#[derive(Default)]
pub(crate) struct ForwardedReferenceCache {
    forwarded_references: HashMap<NamedReference, NamedReference>,
}

pub(crate) enum InheritedExpression {
    Expression(Expression),
    TwoWayBinding,
    Unbound,
}

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
                return InheritedExpression::TwoWayBinding;
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

fn rebase_expression_to_instance(
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
