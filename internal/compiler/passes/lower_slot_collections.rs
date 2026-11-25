// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::BuildDiagnostics;
use crate::element_type::{ElementType, PropertyLookupMode};
use crate::expression_tree::{BindingExpression, Callable, Expression, NamedReference};
use crate::expression_types::Unit;
use crate::langtype::Type;
use crate::object_tree::forward_inherited_expression::{
    ForwardedReferenceCache, rebase_expression_to_instance,
};
use crate::object_tree::*;
use crate::symbol_counters::SymbolCounters;
use smol_str::SmolStr;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::{Rc, Weak};

struct Contract {
    owner: Rc<Component>,
    template: Option<ElementRc>,
    metadata: ElementRc,
    slot: DeclaredSlot,
    length: NamedReference,
    projections: Vec<Projection>,
}

impl Contract {
    fn template(&self) -> &ElementRc {
        self.template.as_ref().unwrap()
    }
}

struct Projection {
    template: ElementRc,
}

fn is_member(element: &ElementRc, slot: &DeclaredSlot) -> bool {
    targets_slot(element, slot) && element.borrow().slot_expansion.is_none()
}

fn targets_slot(element: &ElementRc, slot: &DeclaredSlot) -> bool {
    let element = element.borrow();
    !element.slot_collection
        && element.slot_default.as_deref().is_none_or(|name| name == slot.insertion_name())
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
        for slot in component.declared_slots.borrow().iter().filter(|slot| slot.is_collection) {
            let mut template = None;
            let mut projections = Vec::new();
            recurse_elem(&component.root_element, &(), &mut |element, _| {
                if let Some(expansion) = &element.borrow().slot_expansion
                    && expansion.slot_name == slot.name
                {
                    if expansion.projection_slot.is_some() {
                        projections.push(Projection { template: element.clone() });
                    } else {
                        template = Some(element.clone());
                    }
                }
            });
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
                template,
                metadata,
                slot: slot.clone(),
                length,
                projections,
            });
        }
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
            let is_projection = contracts.iter().any(|contract| {
                Rc::ptr_eq(&contract.owner, component)
                    && contract.projections.iter().any(|projection| {
                        projection
                            .template
                            .borrow()
                            .slot_expansion
                            .as_ref()
                            .unwrap()
                            .projection_slot
                            .as_deref()
                            == Some(name.as_str())
                    })
            });
            if !is_collection
                && !is_projection
                && let Some(contract) = contracts
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
                && contracts.iter().any(|c| {
                    inherits(element, &c.owner)
                        || c.slot.default.is_some() && Rc::ptr_eq(element, &c.owner.root_element)
                })
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
    let mut projection_defaults = HashMap::<_, Vec<ElementRc>>::new();
    for group in groups {
        for contract in contracts
            .iter()
            .filter(|c| inherits(&group, &c.owner) || Rc::ptr_eq(&group, &c.owner.root_element))
        {
            if !Rc::ptr_eq(&group, &contract.owner.root_element)
                && let Some(child) = group.borrow().children.iter().find(|child| {
                    targets_slot(child, &contract.slot) && child.borrow().slot_expansion.is_some()
                })
            {
                diag.push_error(
                    "Composing collection slots is not supported yet".into(),
                    &*child.borrow(),
                );
                continue;
            }
            let children = collection_children(contract, &group);
            if children.is_empty()
                && !group
                    .borrow()
                    .empty_slot_assignments
                    .iter()
                    .any(|name| name.as_str() == contract.slot.insertion_name())
            {
                continue;
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
            if !contract.projections.is_empty()
                && let Some(child) = children.iter().find(|child| {
                    child
                        .borrow()
                        .repeated
                        .as_ref()
                        .is_some_and(|repeated| repeated.is_listview.is_some())
                })
            {
                let operation = "Generating collection views";
                diag.push_error(
                    format!("{operation} from virtualized 'ListView' children is not supported; use model data for the views instead"),
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
            set_length(&group, contract.length.name(), length);
            if children.is_empty() {
                for projection in &contract.projections {
                    group.borrow_mut().empty_slot_assignments.push(
                        projection
                            .template
                            .borrow()
                            .slot_expansion
                            .as_ref()
                            .unwrap()
                            .projection_slot
                            .clone()
                            .unwrap(),
                    );
                }
            }
            for (index, child) in children.iter().enumerate() {
                let index = if child.borrow().repeated.is_some() {
                    Expression::RepeaterIndexReference { element: Rc::downgrade(child) }
                } else {
                    Expression::NumberLiteral(index as f64, Unit::None)
                };
                connect(contract, &group, child, index.clone(), symbols, &mut forwarded, diag);
                for projection in &contract.projections {
                    let view = project(
                        contract,
                        &projection.template,
                        &group,
                        child,
                        index.clone(),
                        symbols,
                        &mut forwarded,
                    );
                    if Rc::ptr_eq(&group, &contract.owner.root_element) {
                        projection_defaults
                            .entry(Rc::as_ptr(&projection.template))
                            .or_default()
                            .push(view);
                    } else {
                        group.borrow_mut().children.push(view);
                    }
                }
            }
        }
    }
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
                let defaults = projection_defaults.remove(&Rc::as_ptr(child)).unwrap_or_default();
                let inserted = defaults.len();
                parent.borrow_mut().children.splice(position..=position, defaults);
                for point in component.child_insertion_points.borrow_mut().values_mut() {
                    if Rc::ptr_eq(&point.parent, parent) && point.insertion_index > position {
                        point.insertion_index = point.insertion_index - 1 + inserted;
                    }
                }
            }
        });
    });
}

fn collection_children(contract: &Contract, group: &ElementRc) -> Vec<ElementRc> {
    if Rc::ptr_eq(group, &contract.owner.root_element) {
        let points = contract.owner.child_insertion_points.borrow();
        points[contract.slot.insertion_name()]
            .parent
            .borrow()
            .children
            .iter()
            .filter(|child| {
                child.borrow().slot_default.as_deref() == Some(contract.slot.insertion_name())
            })
            .cloned()
            .collect()
    } else {
        group
            .borrow()
            .children
            .iter()
            .filter(|child| is_member(child, &contract.slot))
            .cloned()
            .collect()
    }
}

fn set_length(group: &ElementRc, name: &SmolStr, length: Expression) {
    let mut binding = BindingExpression::from(length);
    if let Some(existing) = group.borrow().binding(name) {
        binding.merge_with(&existing);
    }
    group.borrow_mut().set_binding(name.clone(), binding);
}

fn project(
    contract: &Contract,
    template: &ElementRc,
    group: &ElementRc,
    child: &ElementRc,
    index: Expression,
    symbols: &SymbolCounters,
    forwarded: &mut ForwardedReferenceCache,
) -> ElementRc {
    let component = group.borrow().enclosing_component.upgrade().unwrap();
    let view = super::inlining::duplicate_element(template, &component);
    let expansion = view.borrow_mut().slot_expansion.take().unwrap();
    let index = if let Some(repeated) = child.borrow().repeated.clone() {
        view.borrow_mut().repeated = Some(repeated);
        view.borrow_mut().collection_source = Some(Rc::downgrade(child));
        Expression::RepeaterIndexReference { element: Rc::downgrade(&view) }
    } else {
        index
    };
    let entry = expansion.entry_element(&view);
    for (name, analysis) in entry.borrow().property_analysis.borrow().iter() {
        let actual_name = child
            .borrow()
            .lookup_property(name, PropertyLookupMode::ComponentLocal)
            .internal_or_resolved_name();
        if analysis.is_set || analysis.is_set_externally {
            NamedReference::new(child, actual_name.clone()).mark_as_set();
        }
        child
            .borrow()
            .property_analysis
            .borrow_mut()
            .entry(actual_name)
            .or_default()
            .merge_with_base(analysis);
    }
    recurse_elem_no_borrow(&view, &(), &mut |element, _| {
        visit_element_expressions(element, |expression, _, _| {
            expression.visit_recursive_mut(&mut |expression| {
                if let Expression::PropertyReference(reference) = expression
                    && Rc::ptr_eq(&reference.element(), &view)
                    && reference.name() == &expansion.index_property
                {
                    *expression = index.clone();
                }
                if let Expression::ElementReference(reference) = expression {
                    if reference.upgrade().is_some_and(|element| Rc::ptr_eq(&element, &entry)) {
                        *reference = Rc::downgrade(child);
                    } else if reference
                        .upgrade()
                        .is_some_and(|element| Rc::ptr_eq(&element, &contract.owner.root_element))
                    {
                        *reference = Rc::downgrade(group);
                    }
                }
            });
        });
        visit_all_named_references_in_element(element, |reference| {
            if Rc::ptr_eq(&reference.element(), &entry) {
                let name = child
                    .borrow()
                    .lookup_property(reference.name(), PropertyLookupMode::ComponentLocal)
                    .internal_or_resolved_name();
                *reference = NamedReference::new(child, name);
            } else {
                rebase_reference(
                    reference,
                    &contract.owner.root_element,
                    group,
                    symbols,
                    forwarded,
                );
            }
        });
    });
    view.borrow_mut().children.retain(|child| !Rc::ptr_eq(child, &entry));
    if child.borrow().slot_default.is_some() {
        view.borrow_mut().slot_default = expansion.projection_slot.clone();
    }
    view.borrow_mut().slot_target = expansion.projection_slot;
    view
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
    let template = contract.template();
    let mapped = Element {
        bindings: template.borrow().bindings.clone(),
        change_callbacks: template.borrow().change_callbacks.clone(),
        property_analysis: template.borrow().property_analysis.clone(),
        debug: child.borrow().debug.clone(),
        enclosing_component: child.borrow().enclosing_component.clone(),
        ..Default::default()
    }
    .make_rc();
    visit_element_expressions(&mapped, |expression, _, _| {
        expression.visit_recursive_mut(&mut |expression| {
            if let Expression::PropertyReference(reference) = expression
                && Rc::ptr_eq(&reference.element(), template)
                && reference.name()
                    == &template.borrow().slot_expansion.as_ref().unwrap().index_property
            {
                *expression = index.clone();
            }
            if let Expression::ElementReference(element) = expression {
                if element.upgrade().is_some_and(|element| Rc::ptr_eq(&element, template)) {
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
        if Rc::ptr_eq(&reference.element(), template) {
            let name = child
                .borrow()
                .lookup_property(reference.name(), PropertyLookupMode::ComponentLocal)
                .internal_or_resolved_name();
            *reference = NamedReference::new(child, name);
        } else {
            rebase_reference(reference, &contract.owner.root_element, group, symbols, forwarded);
        }
    });
    let mut members: BTreeSet<_> =
        mapped.borrow().real_bindings().map(|(name, _)| name.clone()).collect();
    members.extend(mapped.borrow().change_callbacks.keys().cloned());
    members.extend(mapped.borrow().property_analysis.borrow().keys().cloned());
    members.remove(&template.borrow().slot_expansion.as_ref().unwrap().index_property);
    for name in members {
        let actual_name = child
            .borrow()
            .lookup_property(&name, PropertyLookupMode::ComponentLocal)
            .internal_or_resolved_name();
        super::lower_typed_slots::apply_slot_member(
            &mapped,
            &name,
            child,
            actual_name,
            "slot expansion",
            diag,
        );
    }
}

fn rebase_reference(
    reference: &mut NamedReference,
    owner: &ElementRc,
    group: &ElementRc,
    symbols: &SymbolCounters,
    forwarded: &mut ForwardedReferenceCache,
) {
    if Rc::ptr_eq(owner, group) {
        return;
    }
    let original = reference.clone();
    let is_helper_property = !Rc::ptr_eq(&original.element(), owner)
        && Weak::ptr_eq(
            &original.element().borrow().enclosing_component,
            &owner.borrow().enclosing_component,
        )
        && !matches!(original.ty(), Type::Callback(_) | Type::Function(_));
    let mut expression = Expression::PropertyReference(original.clone());
    rebase_expression_to_instance(&mut expression, owner, group, symbols, forwarded);
    if let Expression::PropertyReference(mapped) = expression {
        if is_helper_property {
            let mut owner = owner.borrow_mut();
            owner.property_declarations.get_mut(mapped.name()).unwrap().is_alias = Some(original);
            owner.take_binding(mapped.name());
        }
        *reference = mapped;
    }
}
