// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Inline each object_tree::Component within the main Component

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BindingExpression, Expression, NamedReference};
use crate::langtype::{ElementType, Type};
use crate::object_tree::*;
use crate::parser::SyntaxNode;
use by_address::ByAddress;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum InlineSelection {
    InlineAllComponents,
    InlineOnlyRequiredComponents,
}

pub fn inline(doc: &Document, inline_selection: InlineSelection, diag: &mut BuildDiagnostics) {
    fn inline_components_recursively(
        component: &Rc<Component>,
        roots: &HashSet<ByAddress<Rc<Component>>>,
        inline_selection: InlineSelection,
        diag: &mut BuildDiagnostics,
    ) {
        recurse_elem_no_borrow(&component.root_element, &(), &mut |elem, _| {
            let base = elem.borrow().base_type.clone();
            if let ElementType::Component(c) = base {
                // First, make sure that the component itself is properly inlined
                inline_components_recursively(&c, roots, inline_selection, diag);

                if c.parent_element.upgrade().is_some() {
                    // We should not inline a repeated element
                    return;
                }

                // Inline this component.
                if match inline_selection {
                    InlineSelection::InlineAllComponents => true,
                    InlineSelection::InlineOnlyRequiredComponents => {
                        component_requires_inlining(&c)
                            || element_require_inlining(elem)
                            // We always inline the root in case the element that instantiate this component needs full inlining,
                            // except when the root is a repeater component, which are never inlined.
                            || component.parent_element.upgrade().is_none() && Rc::ptr_eq(elem, &component.root_element)
                            // We always inline other roots as a component can't be both a sub component and a root
                            || roots.contains(&ByAddress(c.clone()))
                    }
                } {
                    inline_element(elem, &c, component, diag);
                }
            }
        });
        component.popup_windows.borrow().iter().for_each(|p| {
            inline_components_recursively(&p.component, roots, inline_selection, diag)
        })
    }
    let mut roots = HashSet::new();
    if inline_selection == InlineSelection::InlineOnlyRequiredComponents {
        for component in doc.exported_roots().chain(doc.popup_menu_impl.iter().cloned()) {
            roots.insert(ByAddress(component.clone()));
        }
    }
    for component in doc.exported_roots().chain(doc.popup_menu_impl.iter().cloned()) {
        inline_components_recursively(&component, &roots, inline_selection, diag);
        let mut init_code = component.init_code.borrow_mut();
        let inlined_init_code = core::mem::take(&mut init_code.inlined_init_code);
        init_code.constructor_code.splice(0..0, inlined_init_code.into_values());
    }
}

fn element_key(e: ElementRc) -> ByAddress<ElementRc> {
    ByAddress(e)
}

type Mapping = HashMap<ByAddress<ElementRc>, ElementRc>;

fn inline_element(
    elem: &ElementRc,
    inlined_component: &Rc<Component>,
    root_component: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    // inlined_component must be the base type of this element
    debug_assert_eq!(elem.borrow().base_type, ElementType::Component(inlined_component.clone()));
    debug_assert!(
        inlined_component.root_element.borrow().repeated.is_none(),
        "root element of a component cannot be repeated"
    );
    debug_assert!(inlined_component.parent_element.upgrade().is_none());

    let mut elem_mut = elem.borrow_mut();
    let priority_delta = 1 + elem_mut.inline_depth;
    elem_mut.base_type = inlined_component.root_element.borrow().base_type.clone();
    elem_mut.property_declarations.extend(
        inlined_component.root_element.borrow().property_declarations.iter().map(|(name, decl)| {
            let mut decl = decl.clone();
            decl.expose_in_public_api = false;
            (name.clone(), decl)
        }),
    );

    for (p, a) in inlined_component.root_element.borrow().property_analysis.borrow().iter() {
        elem_mut.property_analysis.borrow_mut().entry(p.clone()).or_default().merge_with_base(a);
    }

    // states and transitions must be lowered before inlining
    debug_assert!(inlined_component.root_element.borrow().states.is_empty());
    debug_assert!(inlined_component.root_element.borrow().transitions.is_empty());

    // Map the old element to the new
    let mut mapping = HashMap::new();
    mapping.insert(element_key(inlined_component.root_element.clone()), elem.clone());

    let mut new_children = Vec::with_capacity(
        elem_mut.children.len() + inlined_component.root_element.borrow().children.len(),
    );
    new_children.extend(
        inlined_component.root_element.borrow().children.iter().map(|x| {
            duplicate_element_with_mapping(x, &mut mapping, root_component, priority_delta)
        }),
    );

    // Copy insertion points so we can extend/adjust without mutating the source component.
    let inlined_cips_orig = inlined_component.child_insertion_points.borrow();
    let mut inlined_cips = inlined_cips_orig.clone();

    // Ensure @children CIP exists if it's missing but the component is a builtin that accepts children.
    // This preserves the implicit-children behavior for builtins without explicit placeholders.
    if !inlined_cips.contains_key("_children") {
        if let Some(builtin) = inlined_component.root_element.borrow().builtin_type() {
            if !builtin.is_non_item_type && !builtin.disallow_global_types_as_child_elements {
                let cip_node = inlined_component
                    .node
                    .as_ref()
                    .map(|n| n.clone().into())
                    .or_else(|| {
                        inlined_component
                            .root_element
                            .borrow()
                            .debug
                            .first()
                            .map(|debug| debug.node.clone().into())
                    })
                    .or_else(|| elem_mut.debug.first().map(|debug| debug.node.clone().into()))
                    .or_else(|| root_component.node.as_ref().map(|n| n.clone().into()))
                    .expect("Missing syntax node for implicit @children insertion point");

                inlined_cips.insert(
                    "_children".into(),
                    ChildrenInsertionPoint {
                        parent: inlined_component.root_element.clone(),
                        insertion_index: inlined_component.root_element.borrow().children.len(),
                        node: cip_node,
                    },
                );
            }
        }
    }

    // Group instance children by slot target (named slot or default _children).
    // This preserves relative order within each slot and allows named slot validation.
    let mut children_by_slot: HashMap<SmolStr, Vec<ElementRc>> = HashMap::new();
    let mut seen_slots: HashSet<SmolStr> = HashSet::new();
    for child in std::mem::take(&mut elem_mut.children) {
        let slot = child
            .borrow()
            .slot_target
            .as_ref()
            .map(|s| crate::parser::normalize_identifier(s))
            .unwrap_or_else(|| "_children".into());

        // Only named slots are single-assignment; default _children can appear multiple times.
        if let Some(target) = &child.borrow().slot_target {
            let normalized = crate::parser::normalize_identifier(target);
            if normalized != "_children" && !seen_slots.insert(normalized.clone()) {
                diag.push_error(
                    format!("Duplicate assignment to slot '{target}'"),
                    &*child.borrow(),
                );
            }
        }

        children_by_slot.entry(slot).or_default().push(child);
    }

    // Validate that all referenced slots exist on the inlined component.
    let mut unknown_slots = Vec::new();
    for (slot_name, children) in &children_by_slot {
        if slot_name == "_children" {
            // Missing default slot diagnostics are emitted earlier while constructing the object tree.
            // Keep the existing behavior and avoid reporting "_children" as an unknown named slot here.
            continue;
        }
        if !inlined_cips.contains_key(slot_name.as_str()) {
            for child in children {
                diag.push_error(
                    format!("Unknown slot '{slot_name}' in '{}'", inlined_component.id),
                    &*child.borrow(),
                );
            }
            unknown_slots.push(slot_name.clone());
        }
    }
    for slot_name in unknown_slots {
        children_by_slot.remove(&slot_name);
    }

    let mut move_children_into_popup = None;
    let forwarded_sources_by_target: HashMap<SmolStr, Vec<SmolStr>> =
        elem_mut.forwarded_slots.iter().fold(HashMap::new(), |mut map, forwarding| {
            map.entry(crate::parser::normalize_identifier(forwarding.target.as_str()))
                .or_default()
                .push(crate::parser::normalize_identifier(forwarding.source.as_str()));
            map
        });

    struct SlotInsertion {
        slot_name: SmolStr,
        insertion_index: usize,
        node: SyntaxNode,
        insertion_element: ElementRc,
        children: Vec<ElementRc>,
    }

    // Collect all insertions per parent element so we can insert in a stable order.
    let mut insertions_by_parent: HashMap<ByAddress<ElementRc>, (ElementRc, Vec<SlotInsertion>)> =
        HashMap::new();

    for (slot_name, inlined_cip) in inlined_cips.iter() {
        let children = children_by_slot.remove(slot_name.as_str()).unwrap_or_default();
        if let Some(insertion_element) = mapping.get(&element_key(inlined_cip.parent.clone())) {
            insertions_by_parent
                .entry(element_key(insertion_element.clone()))
                .or_insert_with(|| (insertion_element.clone(), Vec::new()))
                .1
                .push(SlotInsertion {
                    slot_name: slot_name.as_str().into(),
                    insertion_index: inlined_cip.insertion_index,
                    node: inlined_cip.node.clone(),
                    insertion_element: insertion_element.clone(),
                    children,
                });
        } else if !children.is_empty() {
            // @children was into a PopupWindow (named slots inside popups are not supported).
            debug_assert!(inlined_component.popup_windows.borrow().iter().any(|p| Rc::ptr_eq(
                &p.component,
                &inlined_cip.parent.borrow().enclosing_component.upgrade().unwrap()
            )));
            if slot_name == "_children" {
                move_children_into_popup = Some(children);
            } else {
                diag.push_error(
                    format!("Slot '{slot_name}' is inside a PopupWindow, which is not supported"),
                    &inlined_cip.node,
                );
            }
        }
    }

    // Insert slot children and keep root insertion points in sync.
    let mut insertions_for_parent =
        |insertion_element: &ElementRc, insertions: &mut Vec<SlotInsertion>| {
            insertions.sort_by(|a, b| {
                a.insertion_index
                    .cmp(&b.insertion_index)
                    .then_with(|| a.node.span().offset.cmp(&b.node.span().offset))
                    .then_with(|| a.slot_name.cmp(&b.slot_name))
            });

            let mut offset = 0usize;
            if Rc::ptr_eq(elem, insertion_element) {
                // Insert into the new inlined root children vector.
                for insertion in insertions.drain(..) {
                    let adjusted_index = insertion.insertion_index + offset;
                    let inserted_len = insertion.children.len();
                    if inserted_len > 0 {
                        new_children.splice(adjusted_index..adjusted_index, insertion.children);
                    }

                    let mut root_cips = root_component.child_insertion_points.borrow_mut();
                    for (root_slot_name, cip) in root_cips.iter_mut() {
                        let forwarded_match = forwarded_sources_by_target
                            .get(insertion.slot_name.as_str())
                            .is_some_and(|sources| {
                                sources.iter().any(|source| source == root_slot_name)
                            });
                        if Rc::ptr_eq(&cip.parent, elem)
                            && (root_slot_name.as_str() == insertion.slot_name.as_str()
                                || forwarded_match)
                        {
                            *cip = ChildrenInsertionPoint {
                                parent: insertion.insertion_element.clone(),
                                insertion_index: adjusted_index + cip.insertion_index,
                                node: insertion.node.clone(),
                            };
                        }
                    }
                    if root_cips.is_empty()
                        && Rc::ptr_eq(elem, &root_component.root_element)
                        && insertion.slot_name == "_children"
                    {
                        root_cips.insert(
                            "_children".into(),
                            ChildrenInsertionPoint {
                                parent: insertion.insertion_element.clone(),
                                insertion_index: adjusted_index + inserted_len,
                                node: insertion.node.clone(),
                            },
                        );
                    }

                    offset += inserted_len;
                }
            } else {
                // Insert into a mapped child element (not the inlined root).
                let mut insertion_element_mut = insertion_element.borrow_mut();
                for insertion in insertions.drain(..) {
                    let adjusted_index = insertion.insertion_index + offset;
                    let inserted_len = insertion.children.len();
                    if inserted_len > 0 {
                        insertion_element_mut
                            .children
                            .splice(adjusted_index..adjusted_index, insertion.children);
                    }

                    let mut root_cips = root_component.child_insertion_points.borrow_mut();
                    for (root_slot_name, cip) in root_cips.iter_mut() {
                        let forwarded_match = forwarded_sources_by_target
                            .get(insertion.slot_name.as_str())
                            .is_some_and(|sources| {
                                sources.iter().any(|source| source == root_slot_name)
                            });
                        if Rc::ptr_eq(&cip.parent, elem)
                            && (root_slot_name.as_str() == insertion.slot_name.as_str()
                                || forwarded_match)
                        {
                            *cip = ChildrenInsertionPoint {
                                parent: insertion.insertion_element.clone(),
                                insertion_index: adjusted_index + cip.insertion_index,
                                node: insertion.node.clone(),
                            };
                        }
                    }
                    if root_cips.is_empty()
                        && Rc::ptr_eq(elem, &root_component.root_element)
                        && insertion.slot_name == "_children"
                    {
                        root_cips.insert(
                            "_children".into(),
                            ChildrenInsertionPoint {
                                parent: insertion.insertion_element.clone(),
                                insertion_index: adjusted_index + inserted_len,
                                node: insertion.node.clone(),
                            },
                        );
                    }

                    offset += inserted_len;
                }
            }
        };

    for (insertion_element, mut insertions) in insertions_by_parent.into_values() {
        insertions_for_parent(&insertion_element, &mut insertions);
    }

    elem_mut.children = new_children;
    elem_mut.debug.extend_from_slice(&inlined_component.root_element.borrow().debug);

    if let ElementType::Component(c) = &mut elem_mut.base_type
        && c.parent_element.upgrade().is_some()
    {
        debug_assert!(Rc::ptr_eq(elem, &c.parent_element.upgrade().unwrap()));
        *c = duplicate_sub_component(c, elem, &mut mapping, priority_delta);
    };

    root_component.optimized_elements.borrow_mut().extend(
        inlined_component.optimized_elements.borrow().iter().map(|x| {
            duplicate_element_with_mapping(x, &mut mapping, root_component, priority_delta)
        }),
    );
    root_component.popup_windows.borrow_mut().extend(
        inlined_component
            .popup_windows
            .borrow()
            .iter()
            .map(|p| duplicate_popup(p, &mut mapping, priority_delta)),
    );

    root_component.menu_item_tree.borrow_mut().extend(
        inlined_component
            .menu_item_tree
            .borrow()
            .iter()
            .map(|it| duplicate_sub_component(it, elem, &mut mapping, priority_delta)),
    );

    root_component.timers.borrow_mut().extend(inlined_component.timers.borrow().iter().map(|t| {
        let inlined_element = mapping.get(&element_key(t.element.upgrade().unwrap())).unwrap();

        Timer { element: Rc::downgrade(inlined_element), ..t.clone() }
    }));

    let mut moved_into_popup = HashSet::new();
    if let Some(children) = move_children_into_popup {
        let inlined_cips = inlined_component.child_insertion_points.borrow();
        let inlined_cip = inlined_cips.get("_children").unwrap();

        let insertion_element = mapping.get(&element_key(inlined_cip.parent.clone())).unwrap();
        debug_assert!(!std::rc::Weak::ptr_eq(
            &insertion_element.borrow().enclosing_component,
            &elem_mut.enclosing_component,
        ));
        debug_assert!(root_component.popup_windows.borrow().iter().any(|p| Rc::ptr_eq(
            &p.component,
            &insertion_element.borrow().enclosing_component.upgrade().unwrap()
        )));
        for c in &children {
            recurse_elem(c, &(), &mut |e, _| {
                e.borrow_mut().enclosing_component =
                    insertion_element.borrow().enclosing_component.clone();
                moved_into_popup.insert(element_key(e.clone()));
            });
        }
        insertion_element
            .borrow_mut()
            .children
            .splice(inlined_cip.insertion_index..inlined_cip.insertion_index, children);
        let mut root_cips = root_component.child_insertion_points.borrow_mut();
        for cip in root_cips.values_mut() {
            if Rc::ptr_eq(&cip.parent, elem) {
                *cip = ChildrenInsertionPoint {
                    parent: insertion_element.clone(),
                    insertion_index: inlined_cip.insertion_index + cip.insertion_index,
                    node: inlined_cip.node.clone(),
                };
            }
        }
        if root_cips.is_empty() {
            root_cips.insert(
                "_children".into(),
                ChildrenInsertionPoint {
                    parent: insertion_element.clone(),
                    insertion_index: inlined_cip.insertion_index,
                    node: inlined_cip.node.clone(),
                },
            );
        };
    }

    for (k, val) in inlined_component.root_element.borrow().bindings.iter() {
        match elem_mut.bindings.entry(k.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                let priority = &mut entry.insert(val.clone()).get_mut().priority;
                *priority = priority.saturating_add(priority_delta);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut().get_mut();
                if entry.merge_with(&val.borrow()) {
                    entry.priority = entry.priority.saturating_add(priority_delta);
                }
            }
        }
    }
    for (k, val) in inlined_component.root_element.borrow().change_callbacks.iter() {
        match elem_mut.change_callbacks.entry(k.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(val.clone());
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                entry.get_mut().get_mut().splice(0..0, val.borrow().iter().cloned());
            }
        }
    }

    if let Some(orig) = &inlined_component.root_element.borrow().layout_info_prop {
        if let Some(_new) = &mut elem_mut.layout_info_prop {
            todo!("Merge layout infos");
        } else {
            elem_mut.layout_info_prop = Some(orig.clone());
        }
    }

    core::mem::drop(elem_mut);

    let fixup_init_expression = |mut init_code: Expression| {
        // Fix up any property references from within already collected init code.
        visit_named_references_in_expression(&mut init_code, &mut |nr| {
            fixup_reference(nr, &mapping)
        });
        fixup_element_references(&mut init_code, &mapping);
        init_code
    };
    let inlined_init_code = inlined_component
        .init_code
        .borrow()
        .inlined_init_code
        .values()
        .cloned()
        .chain(inlined_component.init_code.borrow().constructor_code.iter().cloned())
        .map(fixup_init_expression)
        .collect();

    root_component
        .init_code
        .borrow_mut()
        .inlined_init_code
        .insert(elem.borrow().span().offset, Expression::CodeBlock(inlined_init_code));

    // Now fixup all binding and reference
    for e in mapping.values() {
        visit_all_named_references_in_element(e, |nr| fixup_reference(nr, &mapping));
        visit_element_expressions(e, |expr, _, _| fixup_element_references(expr, &mapping));
    }
    for p in root_component.popup_windows.borrow_mut().iter_mut() {
        fixup_reference(&mut p.x, &mapping);
        fixup_reference(&mut p.y, &mapping);
    }
    for t in root_component.timers.borrow_mut().iter_mut() {
        fixup_reference(&mut t.interval, &mapping);
        fixup_reference(&mut t.running, &mapping);
        fixup_reference(&mut t.triggered, &mapping);
    }
    // If some element were moved into PopupWindow, we need to report error if they are used outside of the popup window.
    if !moved_into_popup.is_empty() {
        recurse_elem_no_borrow(&root_component.root_element.clone(), &(), &mut |e, _| {
            if !moved_into_popup.contains(&element_key(e.clone())) {
                visit_all_named_references_in_element(e, |nr| {
                    if moved_into_popup.contains(&element_key(nr.element())) {
                        diag.push_error(format!("Access to property '{nr:?}' which is inlined into a PopupWindow via @children is forbidden"), &*e.borrow());
                    }
                });
            }
        });
    }
}

// Duplicate the element elem and all its children. And fill the mapping to point from the old to the new
fn duplicate_element_with_mapping(
    element: &ElementRc,
    mapping: &mut Mapping,
    root_component: &Rc<Component>,
    priority_delta: i32,
) -> ElementRc {
    let elem = element.borrow();
    let new = Rc::new(RefCell::new(Element {
        base_type: elem.base_type.clone(),
        id: elem.id.clone(),
        property_declarations: elem.property_declarations.clone(),
        // We will do the fixup of the references in bindings later
        bindings: elem
            .bindings
            .iter()
            .map(|b| duplicate_binding(b, mapping, root_component, priority_delta))
            .collect(),
        change_callbacks: elem.change_callbacks.clone(),
        property_analysis: elem.property_analysis.clone(),
        children: elem
            .children
            .iter()
            .map(|x| duplicate_element_with_mapping(x, mapping, root_component, priority_delta))
            .collect(),
        repeated: elem.repeated.clone(),
        is_component_placeholder: elem.is_component_placeholder,
        debug: elem.debug.clone(),
        enclosing_component: Rc::downgrade(root_component),
        states: elem.states.clone(),
        transitions: elem
            .transitions
            .iter()
            .map(|t| duplicate_transition(t, mapping, root_component, priority_delta))
            .collect(),
        child_of_layout: elem.child_of_layout,
        layout_info_prop: elem.layout_info_prop.clone(),
        default_fill_parent: elem.default_fill_parent,
        accessibility_props: elem.accessibility_props.clone(),
        geometry_props: elem.geometry_props.clone(),
        named_references: Default::default(),
        item_index: Default::default(), // Not determined yet
        item_index_of_first_children: Default::default(),
        is_flickable_viewport: elem.is_flickable_viewport,
        has_popup_child: elem.has_popup_child,
        is_legacy_syntax: elem.is_legacy_syntax,
        inline_depth: elem.inline_depth + 1,
        slot_target: elem.slot_target.clone(),
        forwarded_slots: elem.forwarded_slots.clone(),
        // Deep-clone grid_layout_cell to avoid sharing between original and inlined copies.
        // This is important because children_constraints contain NamedReferences that need
        // to be fixed up independently for each inlined copy.
        grid_layout_cell: elem
            .grid_layout_cell
            .as_ref()
            .map(|cell| Rc::new(RefCell::new(cell.borrow().clone()))),
    }));
    mapping.insert(element_key(element.clone()), new.clone());
    if let ElementType::Component(c) = &mut new.borrow_mut().base_type
        && c.parent_element.upgrade().is_some()
    {
        debug_assert!(Rc::ptr_eq(element, &c.parent_element.upgrade().unwrap()));
        *c = duplicate_sub_component(c, &new, mapping, priority_delta);
    };

    new
}

/// Duplicate Component for repeated element or popup window that have a parent_element
fn duplicate_sub_component(
    component_to_duplicate: &Rc<Component>,
    new_parent: &ElementRc,
    mapping: &mut Mapping,
    priority_delta: i32,
) -> Rc<Component> {
    debug_assert!(component_to_duplicate.parent_element.upgrade().is_some());
    let new_component = Component {
        node: component_to_duplicate.node.clone(),
        id: component_to_duplicate.id.clone(),
        root_element: duplicate_element_with_mapping(
            &component_to_duplicate.root_element,
            mapping,
            component_to_duplicate, // that's the wrong one, but we fixup further
            priority_delta,
        ),
        parent_element: Rc::downgrade(new_parent),
        optimized_elements: RefCell::new(
            component_to_duplicate
                .optimized_elements
                .borrow()
                .iter()
                .map(|e| {
                    duplicate_element_with_mapping(
                        e,
                        mapping,
                        component_to_duplicate,
                        priority_delta,
                    )
                })
                .collect(),
        ),
        root_constraints: component_to_duplicate.root_constraints.clone(),
        child_insertion_points: component_to_duplicate.child_insertion_points.clone(),
        declared_slots: component_to_duplicate.declared_slots.clone(),
        init_code: component_to_duplicate.init_code.clone(),
        popup_windows: Default::default(),
        timers: component_to_duplicate.timers.clone(),
        menu_item_tree: Default::default(),
        exported_global_names: component_to_duplicate.exported_global_names.clone(),
        used: component_to_duplicate.used.clone(),
        private_properties: Default::default(),
        inherits_popup_window: core::cell::Cell::new(false),
        from_library: core::cell::Cell::new(false),
    };

    let new_component = Rc::new(new_component);
    let weak = Rc::downgrade(&new_component);
    recurse_elem(&new_component.root_element, &(), &mut |e, _| {
        e.borrow_mut().enclosing_component = weak.clone()
    });
    for o in new_component.optimized_elements.borrow().iter() {
        o.borrow_mut().enclosing_component = weak.clone()
    }
    *new_component.popup_windows.borrow_mut() = component_to_duplicate
        .popup_windows
        .borrow()
        .iter()
        .map(|p| duplicate_popup(p, mapping, priority_delta))
        .collect();
    for p in new_component.popup_windows.borrow_mut().iter_mut() {
        fixup_reference(&mut p.x, mapping);
        fixup_reference(&mut p.y, mapping);
    }
    for t in new_component.timers.borrow_mut().iter_mut() {
        fixup_reference(&mut t.interval, mapping);
        fixup_reference(&mut t.running, mapping);
        fixup_reference(&mut t.triggered, mapping);
    }
    *new_component.menu_item_tree.borrow_mut() = component_to_duplicate
        .menu_item_tree
        .borrow()
        .iter()
        .map(|it| {
            let new_parent =
                mapping.get(&element_key(it.parent_element.upgrade().unwrap())).unwrap().clone();
            duplicate_sub_component(it, &new_parent, mapping, priority_delta)
        })
        .collect();
    new_component
        .root_constraints
        .borrow_mut()
        .visit_named_references(&mut |nr| fixup_reference(nr, mapping));
    new_component
}

fn duplicate_popup(p: &PopupWindow, mapping: &mut Mapping, priority_delta: i32) -> PopupWindow {
    let parent = mapping
        .get(&element_key(p.component.parent_element.upgrade().expect("must have a parent")))
        .expect("Parent must be in the mapping")
        .clone();
    PopupWindow {
        x: p.x.clone(),
        y: p.y.clone(),
        close_policy: p.close_policy.clone(),
        component: duplicate_sub_component(&p.component, &parent, mapping, priority_delta),
        parent_element: mapping
            .get(&element_key(p.parent_element.clone()))
            .expect("Parent element must be in the mapping")
            .clone(),
    }
}

/// Clone and increase the priority of a binding
/// and duplicate its animation
fn duplicate_binding(
    (k, b): (&SmolStr, &RefCell<BindingExpression>),
    mapping: &mut Mapping,
    root_component: &Rc<Component>,
    priority_delta: i32,
) -> (SmolStr, RefCell<BindingExpression>) {
    let b = b.borrow();
    let b = BindingExpression {
        expression: b.expression.clone(),
        span: b.span.clone(),
        priority: b.priority.saturating_add(priority_delta),
        animation: b
            .animation
            .as_ref()
            .map(|pa| duplicate_property_animation(pa, mapping, root_component, priority_delta)),
        analysis: b.analysis.clone(),
        two_way_bindings: b.two_way_bindings.clone(),
    };
    (k.clone(), b.into())
}

fn duplicate_property_animation(
    v: &PropertyAnimation,
    mapping: &mut Mapping,
    root_component: &Rc<Component>,
    priority_delta: i32,
) -> PropertyAnimation {
    match v {
        PropertyAnimation::Static(a) => PropertyAnimation::Static(duplicate_element_with_mapping(
            a,
            mapping,
            root_component,
            priority_delta,
        )),
        PropertyAnimation::Transition { state_ref, animations } => PropertyAnimation::Transition {
            state_ref: state_ref.clone(),
            animations: animations
                .iter()
                .map(|a| TransitionPropertyAnimation {
                    state_id: a.state_id,
                    direction: a.direction,
                    animation: duplicate_element_with_mapping(
                        &a.animation,
                        mapping,
                        root_component,
                        priority_delta,
                    ),
                })
                .collect(),
        },
    }
}

fn fixup_reference(nr: &mut NamedReference, mapping: &Mapping) {
    if let Some(e) = mapping.get(&element_key(nr.element())) {
        *nr = NamedReference::new(e, nr.name().clone());
    }
}

fn fixup_element_references(expr: &mut Expression, mapping: &Mapping) {
    let fx = |element: &mut std::rc::Weak<RefCell<Element>>| {
        if let Some(e) = element.upgrade().and_then(|e| mapping.get(&element_key(e))) {
            *element = Rc::downgrade(e);
        }
    };
    let fxe = |element: &mut ElementRc| {
        if let Some(e) = mapping.get(&element_key(element.clone())) {
            *element = e.clone();
        }
    };
    match expr {
        Expression::ElementReference(element) => fx(element),
        Expression::SolveBoxLayout(l, _) | Expression::ComputeBoxLayoutInfo(l, _) => {
            for e in &mut l.elems {
                fxe(&mut e.element);
            }
        }
        Expression::SolveGridLayout { layout, .. }
        | Expression::OrganizeGridLayout(layout)
        | Expression::ComputeGridLayoutInfo { layout, .. } => {
            for e in &mut layout.elems {
                fxe(&mut e.item.element);
            }
        }
        Expression::RepeaterModelReference { element }
        | Expression::RepeaterIndexReference { element } => fx(element),
        _ => expr.visit_mut(|e| fixup_element_references(e, mapping)),
    }
}

fn duplicate_transition(
    t: &Transition,
    mapping: &mut HashMap<ByAddress<ElementRc>, Rc<RefCell<Element>>>,
    root_component: &Rc<Component>,
    priority_delta: i32,
) -> Transition {
    Transition {
        direction: t.direction,
        state_id: t.state_id.clone(),
        property_animations: t
            .property_animations
            .iter()
            .map(|(r, loc, anim)| {
                (
                    r.clone(),
                    loc.clone(),
                    duplicate_element_with_mapping(anim, mapping, root_component, priority_delta),
                )
            })
            .collect(),
        node: t.node.clone(),
    }
}

// Some components need to be inlined to avoid increased complexity in handling them
// in the code generators and subsequent passes.
fn component_requires_inlining(component: &Rc<Component>) -> bool {
    let root_element = &component.root_element;
    if super::flickable::is_flickable_element(root_element) {
        return true;
    }

    for (prop, binding) in &root_element.borrow().bindings {
        let binding = binding.borrow();
        // The passes that dp the drop shadow or the opacity currently won't allow this property
        // on the top level of a component. This could be changed in the future.
        if prop.starts_with("drop-shadow-")
            || prop == "opacity"
            || prop == "cache-rendering-hint"
            || prop == "visible"
        {
            return true;
        }
        if (prop == "height" || prop == "width") && binding.expression.ty() == Type::Percent {
            // percentage size in the root element might not make sense anyway.
            return true;
        }
        if binding.animation.is_some() {
            let lookup_result = root_element.borrow().lookup_property(prop);
            if !lookup_result.is_valid()
                || !lookup_result.is_local_to_component
                || !matches!(
                    lookup_result.property_visibility,
                    PropertyVisibility::Private | PropertyVisibility::Output
                )
            {
                // If there is an animation, we currently inline so that if this property
                // is set with a binding, it is merged
                return true;
            }
        }
    }

    false
}

fn element_require_inlining(elem: &ElementRc) -> bool {
    if !elem.borrow().children.is_empty() {
        // the generators assume that the children list is complete, which sub-components may break
        return true;
    }

    if !elem.borrow().forwarded_slots.is_empty() {
        // Slot forwarding relies on slot insertion points being materialized in this element's
        // subtree, which currently only happens through inlining.
        return true;
    }

    // Popup windows need to be inlined for root.close() to work properly.
    if super::lower_popups::is_popup_window(elem) {
        return true;
    }

    for (prop, binding) in &elem.borrow().bindings {
        if prop == "clip" {
            // otherwise the children of the clipped items won't get moved as child of the Clip element
            return true;
        }

        if (prop == "padding"
            || prop == "spacing"
            || prop.starts_with("padding-")
            || prop.starts_with("spacing-")
            || prop == "alignment")
            && let ElementType::Component(base) = &elem.borrow().base_type
            && crate::layout::is_layout(&base.root_element.borrow().base_type)
            && !base.root_element.borrow().is_binding_set(prop, false)
        {
            // The layout pass need to know that this property is set
            return true;
        }

        let binding = binding.borrow();
        if binding.animation.is_some() && matches!(binding.expression, Expression::Invalid) {
            // If there is an animation but no binding, we must merge the binding with its animation.
            return true;
        }
    }

    false
}
