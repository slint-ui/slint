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
use crate::parser::syntax_nodes;
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
    forwarding: Option<Forwarding>,
    indexed_members: Vec<IndexedMember>,
}

impl Contract {
    fn template(&self) -> &ElementRc {
        self.template.as_ref().unwrap()
    }
}

#[derive(Clone)]
struct Forwarding {
    target: usize,
    group: ElementRc,
    node: syntax_nodes::Expression,
}

struct Composition {
    source: usize,
    target: usize,
    group: ElementRc,
    template: ElementRc,
}

struct Projection {
    template: ElementRc,
    source_projection: Option<usize>,
}

#[derive(Clone)]
struct IndexedMember {
    name: SmolStr,
    function: NamedReference,
    source_projection: Option<usize>,
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
                        projections.push(Projection {
                            template: element.clone(),
                            source_projection: None,
                        });
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
                forwarding: None,
                indexed_members: Vec::new(),
            });
        }
    });
    if contracts.is_empty() {
        return;
    }
    for contract in &mut contracts {
        let names = contract
            .metadata
            .borrow()
            .property_declarations
            .keys()
            .filter(|name| name.starts_with(slot_collections::INDEXED_MEMBER_PREFIX))
            .cloned()
            .collect::<Vec<_>>();
        for name in names {
            let function = slot_collections::prepare_indexed_member(
                &contract.metadata,
                &NamedReference::new(&contract.metadata, name.clone()),
                symbols,
            );
            contract.indexed_members.push(IndexedMember {
                name: name.strip_prefix(slot_collections::INDEXED_MEMBER_PREFIX).unwrap().into(),
                function,
                source_projection: None,
            });
        }
    }
    for source in 0..contracts.len() {
        if contracts[source].template.is_some() {
            continue;
        }
        let point = contracts[source]
            .owner
            .child_insertion_points
            .borrow()
            .get(contracts[source].slot.insertion_name())
            .cloned();
        let Some(point) = point else { continue };
        let forwarding = point
            .parent
            .borrow()
            .forwarded_slots
            .iter()
            .find(|forwarding| forwarding.source == contracts[source].slot.name)
            .cloned();
        let Some(forwarding) = forwarding else { continue };
        let Some(target) = contracts.iter().position(|contract| {
            inherits(&point.parent, &contract.owner) && contract.slot.name == forwarding.target
        }) else {
            continue;
        };
        contracts[source].forwarding =
            Some(Forwarding { target, group: point.parent, node: forwarding.expression_node });
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
                        || (c.slot.default.is_some() || c.forwarding.is_some())
                            && Rc::ptr_eq(element, &c.owner.root_element)
                })
            {
                groups.push(element.clone());
            }
        });
        visit_all_named_references(component, &mut |reference| {
            if let Some(contract) =
                contracts.iter().find(|c| Rc::ptr_eq(&reference.element(), &c.metadata))
                && reference.name().starts_with(slot_collections::INDEXED_MEMBER_PREFIX)
            {
                *reference = contract.metadata.borrow().property_declarations[reference.name()]
                    .is_alias
                    .clone()
                    .unwrap();
                return;
            }
            if let Some(contract) = contracts.iter().find(|c| {
                Rc::ptr_eq(&reference.element(), &c.metadata) && reference.name() == "length"
            }) {
                *reference = contract.length.clone();
            }
        });
    });
    let diagnostics_before_compositions = diag.iter().count();
    let mut compositions = Vec::new();
    for group in &groups {
        for (target, contract) in
            contracts.iter().enumerate().filter(|(_, c)| inherits(group, &c.owner))
        {
            let children: Vec<_> = group
                .borrow()
                .children
                .iter()
                .filter(|child| targets_slot(child, &contract.slot))
                .cloned()
                .collect();
            for child in &children {
                if child.borrow().slot_expansion.is_none() {
                    continue;
                }
                let Some(source) = contracts.iter().position(|source| {
                    source.template.as_ref().is_some_and(|template| Rc::ptr_eq(template, child))
                        || source
                            .projections
                            .iter()
                            .any(|projection| Rc::ptr_eq(&projection.template, child))
                }) else {
                    continue;
                };
                let source_slot = &contracts[source].slot;
                let supplied = children
                    .iter()
                    .filter(|child| {
                        source_slot.default.is_none()
                            || child.borrow().slot_default.as_deref()
                                != Some(source_slot.insertion_name())
                    })
                    .count();
                if supplied != 1 {
                    diag.push_error(
                        "A collection expansion must be the only child in its target collection"
                            .into(),
                        &*child.borrow(),
                    );
                    continue;
                }
                compositions.push(Composition {
                    source,
                    target,
                    group: group.clone(),
                    template: child.clone(),
                });
            }
        }
    }
    let mut prepared = HashSet::new();
    let mut active = HashSet::new();
    let mut forwarded = ForwardedReferenceCache::default();
    for source in 0..contracts.len() {
        prepare_contract(
            source,
            &mut contracts,
            &compositions,
            &mut prepared,
            &mut active,
            symbols,
            &mut forwarded,
            diag,
        );
    }
    if diag
        .iter()
        .skip(diagnostics_before_compositions)
        .any(|diagnostic| diagnostic.level() == crate::diagnostics::DiagnosticLevel::Error)
    {
        return;
    }
    let mut projection_defaults = HashMap::<_, Vec<ElementRc>>::new();
    for group in groups {
        for (contract_index, contract) in contracts.iter().enumerate().filter(|(_, c)| {
            inherits(&group, &c.owner) || Rc::ptr_eq(&group, &c.owner.root_element)
        }) {
            if compositions.iter().any(|composition| {
                composition.target == contract_index && Rc::ptr_eq(&composition.group, &group)
            }) || contracts.iter().any(|source| {
                source.forwarding.as_ref().is_some_and(|forwarding| {
                    forwarding.target == contract_index && Rc::ptr_eq(&forwarding.group, &group)
                })
            }) {
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
            if !validate_consumers(contract_index, &children, &contracts, &compositions, diag) {
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
            if (!contract.projections.is_empty() || !contract.indexed_members.is_empty())
                && let Some(child) = children.iter().find(|child| {
                    child
                        .borrow()
                        .repeated
                        .as_ref()
                        .is_some_and(|repeated| repeated.is_listview.is_some())
                })
            {
                let operation = if contract.projections.is_empty() {
                    "Reading collection entries by index"
                } else {
                    "Generating collection views"
                };
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
            let mut projected_children = vec![Vec::new(); contract.projections.len()];
            for (index, child) in children.iter().enumerate() {
                let index = if child.borrow().repeated.is_some() {
                    Expression::RepeaterIndexReference { element: Rc::downgrade(child) }
                } else {
                    Expression::NumberLiteral(index as f64, Unit::None)
                };
                connect(contract, &group, child, index.clone(), symbols, &mut forwarded, diag);
                let mut views: Vec<ElementRc> = Vec::with_capacity(contract.projections.len());
                for (projection_index, projection) in contract.projections.iter().enumerate() {
                    let source_element = match projection.source_projection {
                        Some(index) => views
                            .get(index)
                            .expect("source projection must precede its dependent view"),
                        None => child,
                    };
                    let view = project(
                        contract,
                        &projection.template,
                        &group,
                        source_element,
                        index.clone(),
                        symbols,
                        &mut forwarded,
                    );
                    views.push(view.clone());
                    projected_children[projection_index].push(view.clone());
                    if Rc::ptr_eq(&group, &contract.owner.root_element) {
                        if let Some(target) = &projection.template.borrow().slot_target {
                            let name = projection
                                .template
                                .borrow()
                                .slot_expansion
                                .as_ref()
                                .unwrap()
                                .projection_slot
                                .clone()
                                .unwrap();
                            let point = contract.owner.child_insertion_points.borrow()
                                [name.as_str()]
                            .clone();
                            view.borrow_mut().slot_target = Some(target.clone());
                            point.parent.borrow_mut().children.push(view);
                        } else {
                            projection_defaults
                                .entry(Rc::as_ptr(&projection.template))
                                .or_default()
                                .push(view);
                        }
                    } else {
                        group.borrow_mut().children.push(view);
                    }
                }
            }
            connect_indexed_members(contract, &group, &children, &projected_children);
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

fn validate_composed_children(
    index: usize,
    children: &[ElementRc],
    contracts: &[Contract],
    compositions: &[Composition],
    diag: &mut BuildDiagnostics,
) -> bool {
    slot_collections::validate_children(&contracts[index].slot, children, diag)
        && validate_consumers(index, children, contracts, compositions, diag)
}

// The lowering pass rejects cycles in prepare_contract before walking these consumers.
fn validate_consumers(
    index: usize,
    children: &[ElementRc],
    contracts: &[Contract],
    compositions: &[Composition],
    diag: &mut BuildDiagnostics,
) -> bool {
    let mut valid = true;
    if let Some(forwarding) = &contracts[index].forwarding {
        valid &=
            validate_composed_children(forwarding.target, children, contracts, compositions, diag);
    }
    for composition in compositions.iter().filter(|composition| {
        composition.source == index
            && composition
                .template
                .borrow()
                .slot_expansion
                .as_ref()
                .unwrap()
                .projection_slot
                .is_none()
    }) {
        valid &=
            validate_composed_children(composition.target, children, contracts, compositions, diag);
    }
    valid
}

fn prepare_forwarding(
    source: usize,
    contracts: &mut [Contract],
    symbols: &SymbolCounters,
    forwarded: &mut ForwardedReferenceCache,
    diag: &mut BuildDiagnostics,
) -> bool {
    let Some(forwarding) = contracts[source].forwarding.clone() else {
        return true;
    };
    let target = &contracts[forwarding.target];
    let owner = contracts[source].owner.clone();
    let slot = contracts[source].slot.clone();
    if let Some(child) =
        forwarding.group.borrow().children.iter().find(|child| is_member(child, &target.slot))
    {
        diag.push_error(
            format!(
                "Remove the children supplied to forwarded collection slot '{}'",
                target.slot.name
            ),
            &*child.borrow(),
        );
        return false;
    }
    let template = super::inlining::duplicate_element(target.template(), &owner);
    template.borrow_mut().bindings = Default::default();
    template.borrow_mut().change_callbacks.clear();
    template.borrow().property_analysis.borrow_mut().clear();
    template.borrow_mut().slot_expansion.as_mut().unwrap().slot_name = slot.name.clone();
    let index = Expression::PropertyReference(NamedReference::new(
        &template,
        template.borrow().slot_expansion.as_ref().unwrap().index_property.clone(),
    ));
    connect(target, &forwarding.group, &template, index, symbols, forwarded, diag);

    let mut length_binding = BindingExpression::from(Expression::Invalid);
    length_binding
        .two_way_bindings
        .push(NamedReference::new(&forwarding.group, target.length.name().clone()).into());
    owner
        .root_element
        .borrow_mut()
        .set_binding(contracts[source].length.name().clone(), length_binding);

    let defaults = inherited_defaults(target, &forwarding.group);
    if !slot_collections::validate_children(&slot, &defaults, diag) {
        return false;
    }
    if !defaults.is_empty() {
        let default_owner = defaults[0].borrow().enclosing_component.upgrade().unwrap();
        let defaults = super::inlining::duplicate_elements(&defaults, &owner);
        for child in &defaults {
            rebase_element(
                child,
                &default_owner.root_element,
                &forwarding.group,
                symbols,
                forwarded,
            );
            child.borrow_mut().slot_target = Some(target.slot.insertion_name().into());
            child.borrow_mut().slot_default = Some(slot.insertion_name().into());
        }
        forwarding.group.borrow_mut().children.extend(defaults);
    }
    let target_name: SmolStr = target.slot.insertion_name().into();
    for entry in &mut forwarding.group.borrow_mut().forwarded_slots {
        if entry.source == slot.name {
            entry.source = slot.insertion_name().into();
            entry.target = target_name.clone();
        }
    }
    contracts[source].template = Some(template);
    forward_projections(source, &forwarding, None, contracts, symbols, forwarded);
    true
}

fn forward_projections(
    source: usize,
    forwarding: &Forwarding,
    input_projection: Option<usize>,
    contracts: &mut [Contract],
    symbols: &SymbolCounters,
    forwarded: &mut ForwardedReferenceCache,
) {
    let target = &contracts[forwarding.target];
    let owner = &contracts[source].owner;
    let slot = &contracts[source].slot;
    let projection_offset = contracts[source].projections.len();
    let mut projections = Vec::new();
    for projection in &target.projections {
        let view = super::inlining::duplicate_element(&projection.template, owner);
        rebase_element(&view, &target.owner.root_element, &forwarding.group, symbols, forwarded);
        let expansion = view.borrow().slot_expansion.clone().unwrap();
        let target_name = expansion.projection_slot.clone().unwrap();
        let source_name = symbols.generate_name("$projection-forwarded-");
        expansion.entry_element(&view).borrow_mut().id = source_name.clone();
        {
            let mut view = view.borrow_mut();
            view.slot_target = Some(target_name.clone());
            let expansion = view.slot_expansion.as_mut().unwrap();
            expansion.slot_name = slot.name.clone();
            expansion.projection_slot = Some(source_name.clone());
        }
        let mut declaration = slot.clone();
        declaration.name = source_name.clone();
        declaration.is_collection = false;
        declaration.is_default = false;
        declaration.interface = None;
        declaration.component_type = None;
        declaration.default = None;
        owner.declared_slots.borrow_mut().push(declaration);
        owner.child_insertion_points.borrow_mut().insert(
            source_name.to_string(),
            ChildrenInsertionPoint {
                parent: forwarding.group.clone(),
                insertion_index: 0,
                node: ChildInsertionPointNode::SlotForwarding(forwarding.node.clone()),
            },
        );
        forwarding.group.borrow_mut().forwarded_slots.push(SlotForwarding {
            target: target_name,
            source: source_name,
            expression_node: forwarding.node.clone(),
        });
        projections.push(Projection {
            template: view,
            source_projection: projection
                .source_projection
                .map(|index| projection_offset + index)
                .or(input_projection),
        });
    }
    contracts[source].projections.extend(projections);
    forward_indexed_members(
        source,
        forwarding,
        input_projection,
        projection_offset,
        contracts,
        symbols,
    );
}

fn inherited_defaults(contract: &Contract, group: &ElementRc) -> Vec<ElementRc> {
    let mut element = group.clone();
    loop {
        let children = collection_children(contract, &element);
        if !children.is_empty()
            || element
                .borrow()
                .empty_slot_assignments
                .iter()
                .any(|name| name.as_str() == contract.slot.insertion_name())
            || Rc::ptr_eq(&element, &contract.owner.root_element)
        {
            return children;
        }
        let ElementType::Component(base) = element.borrow().base_type.clone() else {
            return Vec::new();
        };
        element = base.root_element.clone();
    }
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

fn forward_indexed_members(
    source: usize,
    forwarding: &Forwarding,
    input_projection: Option<usize>,
    projection_offset: usize,
    contracts: &mut [Contract],
    symbols: &SymbolCounters,
) {
    for member in contracts[forwarding.target].indexed_members.clone() {
        let source_projection =
            member.source_projection.map(|index| projection_offset + index).or(input_projection);
        let source_function = if let Some(existing) =
            contracts[source].indexed_members.iter().find(|existing| {
                existing.name == member.name && existing.source_projection == source_projection
            }) {
            existing.function.clone()
        } else {
            let function = slot_collections::create_indexed_reader(
                &contracts[source].owner.root_element,
                member.function.ty(),
                symbols,
            );
            contracts[source].indexed_members.push(IndexedMember {
                name: member.name,
                function: function.clone(),
                source_projection,
            });
            function
        };
        forwarding.group.borrow_mut().set_binding(
            member.function.name().clone(),
            Expression::FunctionCall {
                function: Callable::Function(source_function),
                arguments: vec![Expression::FunctionParameterReference {
                    index: 0,
                    ty: Type::Int32,
                }],
                source_location: None,
            }
            .into(),
        );
    }
}

fn connect_indexed_members(
    contract: &Contract,
    group: &ElementRc,
    children: &[ElementRc],
    projections: &[Vec<ElementRc>],
) {
    for member in &contract.indexed_members {
        let Type::Function(signature) = member.function.ty() else { unreachable!() };
        let children = member.source_projection.map_or(children, |index| &projections[index]);
        let index = Expression::FunctionParameterReference { index: 0, ty: Type::Int32 };
        let mut result = Expression::default_value_for_type(&signature.return_type);
        for (position, child) in children.iter().enumerate().rev() {
            let name = child
                .borrow()
                .lookup_property(&member.name, PropertyLookupMode::ComponentLocal)
                .internal_or_resolved_name();
            let property = NamedReference::new(child, name);
            if child.borrow().repeated.is_some() {
                result = Expression::IndexedPropertyReference {
                    repeater: Some(Rc::downgrade(child)),
                    property,
                    index: Box::new(index.clone()),
                };
            } else {
                result = Expression::Condition {
                    condition: Box::new(Expression::BinaryExpression {
                        lhs: Box::new(index.clone()),
                        rhs: Box::new(Expression::NumberLiteral(position as f64, Unit::None)),
                        op: '=',
                        source_location: None,
                    }),
                    true_expr: Box::new(Expression::IndexedPropertyReference {
                        repeater: None,
                        property,
                        index: Box::new(index.clone()),
                    }),
                    false_expr: Box::new(result),
                    source_location: None,
                };
            }
        }
        group.borrow_mut().set_binding(member.function.name().clone(), result.into());
    }
}

fn rebase_element(
    element: &ElementRc,
    owner: &ElementRc,
    group: &ElementRc,
    symbols: &SymbolCounters,
    forwarded: &mut ForwardedReferenceCache,
) {
    recurse_elem_no_borrow(element, &(), &mut |element, _| {
        visit_element_expressions(element, |expression, _, _| {
            expression.visit_recursive_mut(&mut |expression| {
                if let Expression::ElementReference(reference) = expression
                    && reference.upgrade().is_some_and(|element| Rc::ptr_eq(&element, owner))
                {
                    *reference = Rc::downgrade(group);
                }
            });
        });
        visit_all_named_references_in_element(element, |reference| {
            rebase_reference(reference, owner, group, symbols, forwarded);
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn prepare_contract(
    source: usize,
    contracts: &mut [Contract],
    compositions: &[Composition],
    prepared: &mut HashSet<usize>,
    active: &mut HashSet<usize>,
    symbols: &SymbolCounters,
    forwarded: &mut ForwardedReferenceCache,
    diag: &mut BuildDiagnostics,
) {
    if prepared.contains(&source) {
        return;
    }
    if !active.insert(source) {
        diag.push_error(
            "Collection expansion composition cannot be recursive".into(),
            &contracts[source].slot.name_node,
        );
        return;
    }
    if let Some(forwarding) = contracts[source].forwarding.clone() {
        prepare_contract(
            forwarding.target,
            contracts,
            compositions,
            prepared,
            active,
            symbols,
            forwarded,
            diag,
        );
        if !prepared.contains(&forwarding.target) {
            return;
        }
    }
    for composition in compositions.iter().filter(|composition| composition.source == source) {
        prepare_contract(
            composition.target,
            contracts,
            compositions,
            prepared,
            active,
            symbols,
            forwarded,
            diag,
        );
        if !prepared.contains(&composition.target) {
            return;
        }
        let target = &contracts[composition.target];
        let expansion = composition.template.borrow().slot_expansion.clone().unwrap();
        if expansion.projection_slot.is_some()
            && !validate_composed_children(
                composition.target,
                std::slice::from_ref(&composition.template),
                contracts,
                compositions,
                diag,
            )
        {
            continue;
        }
        let index = Expression::PropertyReference(NamedReference::new(
            &composition.template,
            expansion.index_property,
        ));
        connect(target, &composition.group, &composition.template, index, symbols, forwarded, diag);
        set_length(
            &composition.group,
            target.length.name(),
            Expression::PropertyReference(contracts[source].length.clone()),
        );
        composition
            .group
            .borrow_mut()
            .empty_slot_assignments
            .push(target.slot.insertion_name().into());
        let name: SmolStr = expansion
            .projection_slot
            .unwrap_or_else(|| contracts[source].slot.insertion_name().into());
        composition.group.borrow_mut().forwarded_slots.push(SlotForwarding {
            target: target.slot.insertion_name().into(),
            source: name,
            expression_node: expansion.node.Expression(),
        });
        for projection in &target.projections {
            composition.group.borrow_mut().empty_slot_assignments.push(
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
        let input_projection = contracts[source]
            .projections
            .iter()
            .position(|projection| Rc::ptr_eq(&projection.template, &composition.template));
        forward_projections(
            source,
            &Forwarding {
                target: composition.target,
                group: composition.group.clone(),
                node: expansion.node.Expression(),
            },
            input_projection,
            contracts,
            symbols,
            forwarded,
        );
    }
    if !prepare_forwarding(source, contracts, symbols, forwarded, diag) {
        return;
    }
    active.remove(&source);
    prepared.insert(source);
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
