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
    interface: Rc<Component>,
    count: NamedReference,
}

fn is_member(element: &ElementRc) -> bool {
    let element = element.borrow();
    element.child_bindings_interface.is_none()
        && element.slot_target.is_none()
        && element.slot_default.is_none()
}

fn inherited_contract<'a>(element: &ElementRc, contracts: &'a [Contract]) -> Option<&'a Contract> {
    let mut base = element.borrow().base_type.clone();
    while let ElementType::Component(component) = base {
        if let Some(contract) =
            contracts.iter().find(|contract| Rc::ptr_eq(&contract.owner, &component))
        {
            return Some(contract);
        }
        base = component.root_element.borrow().base_type.clone();
    }
    None
}

fn has_default_slot(component: &Rc<Component>) -> bool {
    let mut current = component.clone();
    loop {
        if current.child_insertion_points.borrow().contains_key(DEFAULT_SLOT_NAME) {
            return true;
        }
        let next = current.root_element.borrow().base_type.clone();
        let ElementType::Component(next) = next else { return false };
        current = next;
    }
}

pub(super) fn lower_child_bindings(
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
        for child in &component.root_element.borrow().children {
            if let Some(interface) = child.borrow().child_bindings_interface.clone() {
                contracts.push(Contract {
                    owner: component.clone(),
                    template: child.clone(),
                    interface,
                    count: child.borrow().property_declarations["count"].is_alias.clone().unwrap(),
                });
            }
        }
    });
    if contracts.is_empty() {
        return;
    }
    for contract in &contracts {
        if inherited_contract(&contract.owner.root_element, &contracts).is_some() {
            diag.push_error(
                "A component cannot replace inherited child-bindings".into(),
                &*contract.template.borrow(),
            );
        }
        if !has_default_slot(&contract.owner) {
            diag.push_error(
                "Add an @children placeholder for child-bindings".into(),
                &*contract.template.borrow(),
            );
        }
    }
    let mut groups = Vec::new();
    let mut seen = HashSet::new();
    doc.visit_all_used_components(|component| {
        for (name, insertion_point) in component.child_insertion_points.borrow().iter() {
            if !Rc::ptr_eq(&insertion_point.parent, &component.root_element)
                && inherited_contract(&insertion_point.parent, &contracts).is_some()
            {
                let target = insertion_point.parent.borrow().base_type.to_string();
                diag.push_error(
                    format!("{} is not allowed as a child of '{target}'", slot_error_subject(name)),
                    &insertion_point.node,
                );
            }
        }
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |element, _| {
            if inherited_contract(element, &contracts).is_some() && seen.insert(Rc::as_ptr(element))
            {
                groups.push(element.clone());
            }
        });
        visit_all_named_references(component, &mut |reference| {
            for contract in &contracts {
                if Rc::ptr_eq(&reference.element(), &contract.template)
                    && reference.name() == "count"
                {
                    *reference = contract.count.clone();
                    break;
                }
            }
        });
    });
    let mut forwarded = ForwardedReferenceCache::default();
    for group in groups {
        let contract = inherited_contract(&group, &contracts).unwrap();
        let children: Vec<_> =
            group.borrow().children.iter().filter(|child| is_member(child)).cloned().collect();
        let mut base = group.borrow().base_type.clone();
        let mut inherited_children = false;
        while let ElementType::Component(component) = base {
            if Rc::ptr_eq(&component, &contract.owner) {
                break;
            }
            if component.root_element.borrow().children.iter().any(is_member) {
                inherited_children = true;
                break;
            }
            base = component.root_element.borrow().base_type.clone();
        }
        if inherited_children {
            if !children.is_empty() {
                diag.push_error(
                    "Declare managed children on the component subclass, not on its instance"
                        .into(),
                    &*group.borrow(),
                );
                group.borrow_mut().children.retain(|child| !is_member(child));
            }
            continue;
        }
        let mut valid = true;
        for child in &children {
            let node = child.borrow().debug[0].node.clone();
            valid &= interfaces::validate_interface_implementation(
                &child.borrow(),
                &contract.interface.root_element,
                &contract.interface.id,
                &node.into(),
                &ImplementBinding::OnSelf,
                diag,
            );
        }
        if !valid {
            continue;
        }
        if let Some(child) = children.iter().find(|child| {
            child.borrow().repeated.as_ref().is_some_and(|repeated| repeated.is_conditional_element)
        }) {
            diag.push_error(
                "Conditional children are not supported by child-bindings".into(),
                &*child.borrow(),
            );
            continue;
        }
        if children.len() > 1
            && let Some(child) = children.iter().find(|child| child.borrow().repeated.is_some())
        {
            diag.push_error(
                "A 'for' loop must be the only managed child in child-bindings".into(),
                &*child.borrow(),
            );
            continue;
        }
        let count = match children.first().and_then(|child| child.borrow().repeated.clone()) {
            Some(repeated) => Expression::FunctionCall {
                function: Callable::Builtin(crate::builtin_functions::BuiltinFunction::ArrayLength),
                arguments: vec![repeated.model],
                source_location: None,
            },
            None => Expression::NumberLiteral(children.len() as f64, Unit::None),
        };
        group.borrow_mut().set_binding(contract.count.name().clone(), count.into());
        for (index, child) in children.iter().enumerate() {
            let index = if child.borrow().repeated.is_some() {
                Expression::RepeaterIndexReference { element: Rc::downgrade(child) }
            } else {
                Expression::NumberLiteral(index as f64, Unit::None)
            };
            connect(contract, &group, child, index, symbols, &mut forwarded, diag);
        }
    }
    for contract in &contracts {
        let root = &contract.owner.root_element;
        let position = root
            .borrow()
            .children
            .iter()
            .position(|child| Rc::ptr_eq(child, &contract.template))
            .unwrap();
        root.borrow_mut().children.remove(position);
        for insertion_point in contract.owner.child_insertion_points.borrow_mut().values_mut() {
            if Rc::ptr_eq(&insertion_point.parent, root)
                && insertion_point.insertion_index > position
            {
                insertion_point.insertion_index -= 1;
            }
        }
    }
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
                && reference.name() == "index"
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
                    "Callback '{name}' has handlers in both child-bindings and its supplied child"
                ),
                &*child.borrow(),
            );
            continue;
        }
        if child.borrow().binding(&actual_name).is_some() {
            diag.push_error(
                format!("Property '{name}' is managed by child-bindings"),
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
