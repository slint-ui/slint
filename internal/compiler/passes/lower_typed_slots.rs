// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::Expression;
use crate::langtype::{ElementType, PropertyLookupMode, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::interfaces;
use crate::object_tree::{Component, ElementRc, recurse_elem, visit_all_named_references};
use crate::typeregister::TypeRegister;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

pub(super) fn prepare(component: &Rc<Component>, tr: &TypeRegister) {
    recurse_elem(&component.root_element, &(), &mut |element, _| {
        let interface = element.borrow().typed_slot_interface.clone();
        if let Some(interface) = interface {
            let mut element = element.borrow_mut();
            let mut declarations = BTreeMap::new();
            for (name, mut member) in interfaces::declared_members(&interface.root_element) {
                let internal_name = element
                    .lookup_property(&name, PropertyLookupMode::ComponentLocal)
                    .internal_or_resolved_name();
                if internal_name != name {
                    member.declaration.shadowed_name = Some(name.clone());
                    element.shadowing_members.insert(name, internal_name.clone());
                }
                member.declaration.expose_in_public_api = false;
                declarations.insert(internal_name, member.declaration);
            }
            element.property_declarations = declarations;
            element.base_type = tr.empty_type();
        }
    });
}

#[allow(clippy::mutable_key_type)]
pub(super) fn lower(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    fn lower_children(
        parent: &ElementRc,
        mapping: &mut HashMap<NamedReference, NamedReference>,
        removed: &mut Vec<ElementRc>,
        diag: &mut BuildDiagnostics,
    ) {
        let children = std::mem::take(&mut parent.borrow_mut().children);
        let mut result = Vec::new();
        for child in children {
            let base = child.borrow().base_type.clone();
            if let ElementType::Component(base) = base
                && base.parent_element().is_some()
            {
                lower(&base, diag);
            }
            lower_children(&child, mapping, removed, diag);
            if child.borrow().typed_slot_interface.is_none() {
                result.push(child);
                continue;
            }
            if child.borrow().children.len() != 1 {
                let name = child.borrow().id.clone();
                diag.push_error(
                    format!("Typed slot '{name}' requires exactly one component"),
                    &*child.borrow(),
                );
                result.push(child);
                continue;
            }
            let actual = child.borrow_mut().children.remove(0);
            let declarations = child.borrow().property_declarations.clone();
            for (name, declaration) in declarations {
                let actual_name = actual
                    .borrow()
                    .lookup_property(
                        declaration.declared_name(&name),
                        PropertyLookupMode::ComponentLocal,
                    )
                    .internal_or_resolved_name();
                let reference = NamedReference::new(&actual, actual_name.clone());
                mapping.insert(NamedReference::new(&child, name.clone()), reference.clone());
                let binding = child
                    .borrow()
                    .binding_cell_including_synthetic(&name)
                    .map(|binding| binding.borrow().clone());
                if let Some(binding) = binding {
                    if matches!(declaration.property_type, Type::Callback(_))
                        && has_callback_handler(
                            &NamedReference::new(&child, name.clone()),
                            &mut HashSet::new(),
                        )
                        && has_callback_handler(&reference, &mut HashSet::new())
                    {
                        diag.push_error(
                            format!("Callback '{name}' has handlers in both the typed slot and its supplied component"),
                            &*child.borrow(),
                        );
                    }
                    let mut target = actual.borrow_mut();
                    if let Some(existing) = target.binding_cell_including_synthetic(&actual_name) {
                        let mut existing = existing.borrow_mut();
                        if binding.priority < existing.priority && binding.has_binding() {
                            let mut replacement = binding;
                            replacement.merge_with(&existing);
                            *existing = replacement;
                        } else {
                            existing.merge_with(&binding);
                        }
                    } else {
                        target.set_binding(actual_name.clone(), binding);
                    }
                }
                if let Some(handlers) = child.borrow().change_callbacks.get(&name) {
                    actual
                        .borrow_mut()
                        .change_callbacks
                        .entry(actual_name.clone())
                        .or_default()
                        .borrow_mut()
                        .extend(handlers.borrow().iter().cloned());
                }
                if let Some(analysis) = child.borrow().property_analysis.borrow().get(&name) {
                    actual
                        .borrow()
                        .property_analysis
                        .borrow_mut()
                        .entry(actual_name)
                        .or_default()
                        .merge_with_base(analysis);
                }
            }
            removed.push(child);
            result.push(actual);
        }
        parent.borrow_mut().children = result;
    }
    let mut mapping = HashMap::new();
    let mut removed = Vec::new();
    lower_children(&component.root_element, &mut mapping, &mut removed, diag);
    visit_all_named_references(component, &mut |reference| {
        if let Some(target) = mapping.get(reference) {
            *reference = target.clone();
        }
    });
    for popup in component.popup_windows.borrow().iter() {
        lower(&popup.component, diag);
    }
    for menu in component.menu_item_tree.borrow().iter() {
        lower(menu, diag);
    }
}

#[allow(clippy::mutable_key_type)]
fn has_callback_handler(reference: &NamedReference, visited: &mut HashSet<NamedReference>) -> bool {
    if !visited.insert(reference.clone()) {
        return false;
    }
    let element = reference.element();
    let element = element.borrow();
    if let Some(binding) = element.binding(reference.name()) {
        if !matches!(binding.value_expression(), Expression::Invalid) {
            return true;
        }
        for alias in &binding.two_way_bindings {
            if let crate::expression_tree::TwoWayBinding::Property { property, .. } = alias
                && has_callback_handler(property, visited)
            {
                return true;
            }
        }
    }
    if let ElementType::Component(base) = &element.base_type {
        return has_callback_handler(
            &NamedReference::new(&base.root_element, reference.name().clone()),
            visited,
        );
    }
    false
}
