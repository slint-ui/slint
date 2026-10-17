// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Rejects references into the default content of a slot from outside that content,
//! since a caller can replace the content.

use std::collections::{BTreeSet, HashMap, HashSet};

use by_address::ByAddress;
use smol_str::SmolStr;

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Spanned};
use crate::element_type::PropertyLookupMode;
use crate::expression_tree::{Callable, ConditionLocation, Expression, TwoWayBinding};
use crate::object_tree::{
    Document, ElementRc, PropertyAnimation, recurse_elem_no_borrow,
    visit_all_named_references_in_element,
};
use crate::parser::{identifier_text, syntax_nodes};

#[allow(clippy::mutable_key_type)]
pub fn check_slot_defaults(doc: &Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        let mut owners = HashMap::new();
        let mut default_roots = Vec::new();
        recurse_elem_no_borrow(
            &component.root_element,
            &None,
            &mut |element, owner: &Option<SmolStr>| {
                let slot_default = element.borrow().slot_default.clone();
                if slot_default.is_some() {
                    default_roots.push(element.clone());
                }
                let owner = slot_default.or_else(|| owner.clone());
                if let Some(owner) = &owner {
                    owners.insert(ByAddress(element.clone()), owner.clone());
                }
                owner
            },
        );
        if owners.is_empty() {
            continue;
        }
        let mut defaults_kept_for_inlining = BTreeSet::new();
        recurse_elem_no_borrow(&component.root_element, &(), &mut |element, _| {
            let owner = owners.get(&ByAddress(element.clone()));
            let foreign_owner = |target: &ElementRc| {
                owners.get(&ByAddress(target.clone())).filter(|target| Some(*target) != owner)
            };
            let message = |name: &SmolStr| {
                format!(
                    "Move references to the default content of slot '{name}' inside that content"
                )
            };
            let mut references = element_references(element);
            let referenced: HashSet<_> =
                references.iter().map(|(target, _)| ByAddress(target.clone())).collect();
            let location = element.borrow().to_source_location();
            visit_all_named_references_in_element(element, |reference| {
                let target = reference.element();
                if !referenced.contains(&ByAddress(target.clone())) {
                    references.push((target, location.clone()));
                }
            });

            let mut reported = BTreeSet::new();
            for (target, location) in &references {
                if let Some(name) = foreign_owner(target)
                    && reported.insert((name.clone(), location.span.offset, location.span.length))
                {
                    diag.push_error(message(name), location);
                    defaults_kept_for_inlining.insert(name.clone());
                }
            }
        });
        for root in default_roots {
            let mut root = root.borrow_mut();
            if root
                .slot_default
                .as_ref()
                .is_some_and(|name| defaults_kept_for_inlining.contains(name))
            {
                root.slot_default = None;
            }
        }
    }
}

fn element_references(element: &ElementRc) -> Vec<(ElementRc, SourceLocation)> {
    let mut references = binding_references(element);
    let element = element.borrow();
    for transition in &element.transitions {
        for (_, _, animation) in &transition.property_animations {
            references.extend(binding_references(animation));
        }
        if let Some((_, animation)) = &transition.catch_all_property_animation {
            references.extend(binding_references(animation));
        }
    }
    for state in &element.states {
        if let (Some(condition), Some(ConditionLocation::StateSelection { when, .. })) =
            (&state.condition, &state.selection)
        {
            references.extend(expression_targets(condition, when));
        }
        for (property, expression, node) in &state.property_changes {
            let location = node.QualifiedName().to_source_location();
            references.extend(expression_targets(expression, &location));
            references.push((property.element(), location));
        }
    }

    for transition in &element.transitions {
        for (property, location, _) in &transition.property_animations {
            references.push((property.element(), location.clone()));
        }
    }

    if let Some(node) = element.debug.first().map(|debug| &debug.node) {
        for callback in node.PropertyChangedCallback() {
            let Some(property) = identifier_text(&callback.DeclaredIdentifier()) else {
                continue;
            };
            let name = element
                .lookup_property(&property, PropertyLookupMode::ComponentLocal)
                .internal_or_resolved_name();
            let Some(handlers) = element.change_callbacks.get(&name) else { continue };
            let location = match callback.CodeBlock() {
                Some(body) => body.to_source_location(),
                None => callback.Expression().map_or_else(
                    || callback.to_source_location(),
                    |body| body.to_source_location(),
                ),
            };
            for handler in handlers.borrow().iter() {
                references.extend(expression_targets(handler, &location));
            }
        }
    }

    if let Some(repeated) = &element.repeated {
        references.extend(expression_targets(&repeated.model, &element.to_source_location()));
    }
    references
}

fn binding_references(element: &ElementRc) -> Vec<(ElementRc, SourceLocation)> {
    let mut references = Vec::new();
    for (_, binding) in element.borrow().bindings_including_synthetic() {
        let binding = binding.borrow();
        let location = binding.to_source_location();
        references.extend(expression_targets(&binding.expression, &location));
        for two_way in &binding.two_way_bindings {
            if let TwoWayBinding::Property { property, .. } = two_way {
                references.push((property.element(), location.clone()));
            }
        }
        match &binding.animation {
            Some(PropertyAnimation::Static(animation)) => {
                references.extend(binding_references(animation))
            }
            Some(PropertyAnimation::Transition { animations, .. }) => {
                for animation in animations {
                    references.extend(binding_references(&animation.animation));
                }
            }
            None => {}
        }
    }
    references
}

fn expression_targets(
    expression: &Expression,
    fallback: &SourceLocation,
) -> Vec<(ElementRc, SourceLocation)> {
    let mut targets = Vec::new();
    collect_expression_targets(expression, fallback, &mut targets);
    targets
}

fn collect_expression_targets(
    expression: &Expression,
    fallback: &SourceLocation,
    targets: &mut Vec<(ElementRc, SourceLocation)>,
) {
    if let Expression::SelfAssignment { lhs, rhs, node: Some(node), .. } = expression
        && let Some(node) = node.as_node().cloned().and_then(syntax_nodes::SelfAssignment::new)
    {
        let (lhs_node, rhs_node) = node.Expression();
        collect_expression_targets(lhs, &lhs_node.to_source_location(), targets);
        collect_expression_targets(rhs, &rhs_node.to_source_location(), targets);
        return;
    }
    match expression {
        Expression::PropertyReference(reference) => {
            targets.push((reference.element(), fallback.clone()))
        }
        Expression::FunctionCall {
            function: Callable::Callback(reference) | Callable::Function(reference),
            source_location,
            ..
        } => targets
            .push((reference.element(), source_location.as_ref().unwrap_or(fallback).clone())),
        Expression::ElementReference(target) => {
            targets.extend(target.upgrade().map(|target| (target, fallback.clone())))
        }
        _ => {}
    }
    expression.visit(|sub| collect_expression_targets(sub, fallback, targets));
}
