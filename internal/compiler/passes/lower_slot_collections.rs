// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::BuildDiagnostics;
use crate::element_type::{ElementType, PropertyLookupMode};
use crate::expression_tree::{Callable, Expression, NamedReference};
use crate::expression_types::Unit;
use crate::langtype::Type;
use crate::object_tree::forward_inherited_expression::{
    ForwardedReferenceCache, rebase_expression_to_instance,
};
use crate::object_tree::interfaces::{self, ImplementBinding};
use crate::object_tree::*;
use crate::symbol_counters::SymbolCounters;
use smol_str::SmolStr;
use std::collections::HashSet;
use std::rc::Rc;

struct Contract {
    owner: Rc<Component>,
    template: ElementRc,
    metadata: ElementRc,
    slot: DeclaredSlot,
    length: NamedReference,
}

fn is_member(element: &ElementRc, slot: &DeclaredSlot) -> bool {
    let element = element.borrow();
    !element.slot_collection
        && element.slot_expansion.is_none()
        && element.slot_default.is_none()
        && if slot.is_default {
            element.slot_target.is_none()
                || element.slot_target.as_deref() == Some(DEFAULT_SLOT_NAME)
        } else {
            element.slot_target.as_ref() == Some(&slot.name)
        }
}

fn inherits(element: &ElementRc, owner: &Rc<Component>) -> bool {
    let mut base = element.borrow().base_type.clone();
    while let ElementType::Component(component) = base {
        if Rc::ptr_eq(&component, owner) {
            return true;
        }
        base = component.root_element.borrow().base_type.clone();
    }
    false
}

pub(super) fn lower_slot_collections(
    doc: &Document,
    symbols: &SymbolCounters,
    diag: &mut BuildDiagnostics,
) {
    let mut contracts = Vec::new();
    let mut seen_components = HashSet::new();
    doc.visit_all_used_components(|component| {
        if !seen_components.insert(Rc::as_ptr(component)) {
            return;
        }
        recurse_elem(&component.root_element, &(), &mut |element, _| {
            let expansion = element.borrow().slot_expansion.clone();
            let Some(expansion) = expansion else { return };
            let slot = component
                .declared_slots
                .borrow()
                .iter()
                .find(|s| s.name == expansion.slot_name)
                .unwrap()
                .clone();
            let metadata = component
                .root_element
                .borrow()
                .children
                .iter()
                .find(|e| e.borrow().slot_collection && e.borrow().id == slot.name)
                .unwrap()
                .clone();
            let length =
                metadata.borrow().property_declarations["length"].is_alias.clone().unwrap();
            contracts.push(Contract {
                owner: component.clone(),
                template: element.clone(),
                metadata,
                slot,
                length,
            });
        });
    });
    if contracts.is_empty() {
        return;
    }
    let mut groups = Vec::new();
    let mut seen = HashSet::new();
    doc.visit_all_used_components(|component| {
        for (name, point) in component.child_insertion_points.borrow().iter() {
            if Rc::ptr_eq(&point.parent, &component.root_element) {
                continue;
            }
            let is_collection = component
                .declared_slots
                .borrow()
                .iter()
                .any(|slot| slot.is_collection && slot.insertion_name() == name);
            if is_collection
                && contracts.iter().any(|c| c.slot.is_default && inherits(&point.parent, &c.owner))
            {
                diag.push_error(
                    "Expanding a collection into another collection slot is not supported yet"
                        .into(),
                    &point.node.syntax_node().parent().unwrap(),
                );
            } else if let Some(contract) = contracts
                .iter()
                .find(|c| inherits(&point.parent, &c.owner) && c.slot.insertion_name() == name)
            {
                let target = point.parent.borrow().base_type.to_string();
                diag.push_error(
                    format!(
                        "{} is not allowed as a child of '{target}'",
                        slot_error_subject(contract.slot.insertion_name())
                    ),
                    &point.node,
                );
            }
        }
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |element, _| {
            if seen.insert(Rc::as_ptr(element))
                && contracts.iter().any(|c| inherits(element, &c.owner))
            {
                groups.push(element.clone());
            }
        });
        visit_all_named_references(component, &mut |reference| {
            if let Some(contract) = contracts.iter().find(|c| {
                Rc::ptr_eq(&reference.element(), &c.metadata) && reference.name() == "length"
            }) {
                *reference = contract.length.clone();
            }
        });
    });
    let mut forwarded = ForwardedReferenceCache::default();
    for group in groups {
        for contract in contracts.iter().filter(|c| inherits(&group, &c.owner)) {
            let children: Vec<_> = group
                .borrow()
                .children
                .iter()
                .filter(|e| is_member(e, &contract.slot))
                .cloned()
                .collect();
            let mut base = group.borrow().base_type.clone();
            let mut inherited_children = false;
            while let ElementType::Component(component) = base {
                if Rc::ptr_eq(&component, &contract.owner) {
                    break;
                }
                if component
                    .root_element
                    .borrow()
                    .children
                    .iter()
                    .any(|e| is_member(e, &contract.slot))
                {
                    inherited_children = true;
                    break;
                }
                base = component.root_element.borrow().base_type.clone();
            }
            if inherited_children {
                if !children.is_empty() {
                    diag.push_error("Declare collection children on the component subclass, not on its instance".into(), &*group.borrow());
                    group.borrow_mut().children.retain(|e| !is_member(e, &contract.slot));
                }
                continue;
            }
            if let Some(interface) = &contract.slot.interface {
                let mut valid = true;
                for child in &children {
                    let node = child.borrow().debug[0].node.clone();
                    valid &= interfaces::validate_interface_implementation(
                        &child.borrow(),
                        &interface.root_element,
                        &interface.id,
                        &node.into(),
                        &ImplementBinding::OnSelf,
                        diag,
                    );
                }
                if !valid {
                    continue;
                }
            }
            if let Some(child) = children
                .iter()
                .find(|e| e.borrow().repeated.as_ref().is_some_and(|r| r.is_conditional_element))
            {
                diag.push_error(
                    "Conditional children are not supported in collection slots yet".into(),
                    &*child.borrow(),
                );
                continue;
            }
            if children.len() > 1
                && let Some(child) = children.iter().find(|e| e.borrow().repeated.is_some())
            {
                diag.push_error(
                    "A 'for' loop must be the only child in a collection slot".into(),
                    &*child.borrow(),
                );
                continue;
            }
            let length = match children.first().and_then(|e| e.borrow().repeated.clone()) {
                Some(repeated) => Expression::FunctionCall {
                    function: Callable::Builtin(
                        crate::builtin_functions::BuiltinFunction::ArrayLength,
                    ),
                    arguments: vec![repeated.model],
                    source_location: None,
                },
                None => Expression::NumberLiteral(children.len() as f64, Unit::None),
            };
            group.borrow_mut().set_binding(contract.length.name().clone(), length.into());
            for (index, child) in children.iter().enumerate() {
                let index = if child.borrow().repeated.is_some() {
                    Expression::RepeaterIndexReference { element: Rc::downgrade(child) }
                } else {
                    Expression::NumberLiteral(index as f64, Unit::None)
                };
                connect(contract, &group, child, index, symbols, &mut forwarded, diag);
            }
        }
    }
    // The insertion points keep their position while the compile-time templates disappear.
    let mut cleaned = HashSet::new();
    doc.visit_all_used_components(|component| {
        if !cleaned.insert(Rc::as_ptr(component)) {
            return;
        }
        recurse_elem(&component.root_element, &(), &mut |parent, _| {
            let children = parent.borrow().children.clone();
            for (position, child) in children.iter().enumerate().rev() {
                if !child.borrow().slot_collection && child.borrow().slot_expansion.is_none() {
                    continue;
                }
                parent.borrow_mut().children.remove(position);
                for point in component.child_insertion_points.borrow_mut().values_mut() {
                    if Rc::ptr_eq(&point.parent, parent) && point.insertion_index > position {
                        point.insertion_index -= 1;
                    }
                }
            }
        });
    });
}

fn connect(
    contract: &Contract,
    group: &ElementRc,
    child: &ElementRc,
    index: Expression,
    symbols: &SymbolCounters,
    forwarded: &mut ForwardedReferenceCache,
    diag: &mut BuildDiagnostics,
) {
    let mapped = Element {
        bindings: contract.template.borrow().bindings.clone(),
        change_callbacks: contract.template.borrow().change_callbacks.clone(),
        enclosing_component: child.borrow().enclosing_component.clone(),
        ..Default::default()
    }
    .make_rc();
    visit_element_expressions(&mapped, |expression, _, _| {
        expression.visit_recursive_mut(&mut |expression| {
            if let Expression::PropertyReference(reference) = expression
                && Rc::ptr_eq(&reference.element(), &contract.template)
                && reference.name()
                    == &contract.template.borrow().slot_expansion.as_ref().unwrap().index_property
            {
                *expression = index.clone();
            }
            if let Expression::ElementReference(element) = expression {
                if element.upgrade().is_some_and(|element| Rc::ptr_eq(&element, &contract.template))
                {
                    *element = Rc::downgrade(child);
                } else if element
                    .upgrade()
                    .is_some_and(|element| Rc::ptr_eq(&element, &contract.owner.root_element))
                {
                    *element = Rc::downgrade(group);
                }
            }
        });
    });
    visit_all_named_references_in_element(&mapped, |reference| {
        if Rc::ptr_eq(&reference.element(), &contract.template) {
            let name = child
                .borrow()
                .lookup_property(reference.name(), PropertyLookupMode::ComponentLocal)
                .internal_or_resolved_name();
            *reference = NamedReference::new(child, name);
        } else {
            let mut expression = Expression::PropertyReference(reference.clone());
            rebase_expression_to_instance(
                &mut expression,
                &contract.owner.root_element,
                group,
                symbols,
                forwarded,
            );
            if let Expression::PropertyReference(mapped) = expression {
                *reference = mapped;
            }
        }
    });
    for (name, binding) in mapped.borrow().real_bindings() {
        let actual = child.borrow().lookup_property(name, PropertyLookupMode::ComponentLocal);
        let is_callback = matches!(actual.property_type, Type::Callback(_));
        let actual_name: SmolStr = actual.internal_or_resolved_name();
        if is_callback
            && super::lower_typed_slots::has_callback_handler(
                &NamedReference::new(child, actual_name.clone()),
                &mut HashSet::new(),
            )
        {
            diag.push_error(
                format!(
                    "Callback '{name}' has handlers in both the slot expansion and its supplied child"
                ),
                &*child.borrow(),
            );
            continue;
        }
        if child.borrow().binding(&actual_name).is_some() {
            diag.push_error(
                format!("Property '{name}' is managed by the slot expansion"),
                &*child.borrow(),
            );
            continue;
        }
        let mut target = child.borrow_mut();
        if let Some(existing) = target.binding_cell_including_synthetic(&actual_name) {
            existing.borrow_mut().merge_with(&binding.borrow());
        } else {
            target.set_binding(actual_name, binding.borrow().clone());
        }
    }
    for (name, handlers) in &mapped.borrow().change_callbacks {
        let actual_name = child
            .borrow()
            .lookup_property(name, PropertyLookupMode::ComponentLocal)
            .internal_or_resolved_name();
        child
            .borrow_mut()
            .change_callbacks
            .entry(actual_name)
            .or_default()
            .borrow_mut()
            .extend(handlers.borrow().iter().cloned());
    }
}
