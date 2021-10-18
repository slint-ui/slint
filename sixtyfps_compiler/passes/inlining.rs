/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Inline each object_tree::Component within the main Component

use crate::expression_tree::{BindingExpression, Expression, NamedReference};
use crate::langtype::Type;
use crate::object_tree::*;
use by_address::ByAddress;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Copy, Clone)]
pub enum InlineSelection {
    InlineAllComponents,
    #[allow(dead_code)] // allow until it's an option globally used in the compiler
    InlineOnlyRequiredComponents,
}

pub fn inline(doc: &Document, inline_selection: InlineSelection) {
    fn inline_components_recursively(component: &Rc<Component>, inline_selection: InlineSelection) {
        recurse_elem(&component.root_element, &(), &mut |elem, _| {
            let base = elem.borrow().base_type.clone();
            if let Type::Component(c) = base {
                // First, make sure that the component itself is properly inlined
                inline_components_recursively(&c, inline_selection);
                // Inline this component.
                if match inline_selection {
                    InlineSelection::InlineAllComponents => true,
                    InlineSelection::InlineOnlyRequiredComponents => {
                        component_requires_inlining(&c)
                    }
                } {
                    inline_element(elem, &c, component);
                }
            }
        })
    }
    inline_components_recursively(&doc.root_component, inline_selection)
}

fn clone_tuple<U: Clone, V: Clone>((u, v): (&U, &V)) -> (U, V) {
    (u.clone(), v.clone())
}

fn element_key(e: ElementRc) -> ByAddress<ElementRc> {
    ByAddress(e)
}

fn inline_element(
    elem: &ElementRc,
    inlined_component: &Rc<Component>,
    root_component: &Rc<Component>,
) {
    // inlined_component must be the base type of this element
    debug_assert_eq!(
        format!("{:?}", elem.borrow().base_type),
        format!("{:?}", Type::Component(inlined_component.clone()))
    );
    debug_assert!(
        inlined_component.root_element.borrow().repeated.is_none(),
        "root element of a component cannot be repeated"
    );

    let mut elem_mut = elem.borrow_mut();
    elem_mut.base_type = inlined_component.root_element.borrow().base_type.clone();
    elem_mut.property_declarations.extend(
        inlined_component.root_element.borrow().property_declarations.iter().map(clone_tuple),
    );

    for (p, a) in inlined_component.root_element.borrow().property_analysis.borrow().iter() {
        elem_mut.property_analysis.borrow_mut().entry(p.clone()).or_default().merge(a);
    }

    // FIXME: states and transitions will be merged while inlining, this is not what we want
    elem_mut.states.extend(inlined_component.root_element.borrow().states.iter().cloned());
    elem_mut
        .transitions
        .extend(inlined_component.root_element.borrow().transitions.iter().cloned());

    // Map the old element to the new
    let mut mapping = HashMap::new();
    mapping.insert(element_key(inlined_component.root_element.clone()), elem.clone());

    let mut new_children = vec![];
    new_children
        .reserve(elem_mut.children.len() + inlined_component.root_element.borrow().children.len());
    new_children.extend(
        inlined_component
            .root_element
            .borrow()
            .children
            .iter()
            .map(|x| duplicate_element_with_mapping(x, &mut mapping, root_component)),
    );

    match inlined_component
        .child_insertion_point
        .borrow()
        .as_ref()
        .and_then(|(elem, node)| Some((mapping.get(&element_key(elem.clone()))?, node)))
    {
        Some((insertion_element, cip_node)) if !Rc::ptr_eq(elem, insertion_element) => {
            insertion_element.borrow_mut().children.append(&mut elem_mut.children);
            if let Some(cip) = root_component.child_insertion_point.borrow_mut().as_mut() {
                if Rc::ptr_eq(&cip.0, elem) {
                    *cip = (insertion_element.clone(), cip_node.clone());
                }
            };
        }
        _ => {
            new_children.append(&mut elem_mut.children);
        }
    }

    elem_mut.children = new_children;

    for (k, val) in inlined_component.root_element.borrow().bindings.iter() {
        match elem_mut.bindings.entry(k.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(val.clone()).priority += 1;
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if entry.get_mut().merge_with(val) {
                    entry.get_mut().priority += 1;
                }
            }
        }
    }

    core::mem::drop(elem_mut);

    // Now fixup all binding and reference
    for e in mapping.values() {
        visit_all_named_references_in_element(e, |nr| fixup_reference(nr, &mapping));
        visit_element_expressions(e, |expr, _, _| fixup_element_references(expr, &mapping));
    }
}

// Duplicate the element elem and all its children. And fill the mapping to point from the old to the new
fn duplicate_element_with_mapping(
    element: &ElementRc,
    mapping: &mut HashMap<ByAddress<ElementRc>, ElementRc>,
    root_component: &Rc<Component>,
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
            .map(|b| duplicate_binding(b, mapping, root_component))
            .collect(),
        property_analysis: elem.property_analysis.clone(),
        children: elem
            .children
            .iter()
            .map(|x| duplicate_element_with_mapping(x, mapping, root_component))
            .collect(),
        repeated: elem.repeated.clone(),
        node: elem.node.clone(),
        enclosing_component: Rc::downgrade(root_component),
        states: elem.states.clone(),
        transitions: elem
            .transitions
            .iter()
            .map(|t| duplicate_transition(t, mapping, root_component))
            .collect(),
        child_of_layout: elem.child_of_layout,
        layout_info_prop: elem.layout_info_prop.clone(),
        named_references: Default::default(),
        item_index: Default::default(), // Not determined yet
        is_flickable_viewport: elem.is_flickable_viewport,
    }));
    mapping.insert(element_key(element.clone()), new.clone());
    new
}

/// Clone and increase the priority of a binding
/// and duplicate its animation
fn duplicate_binding(
    (k, b): (&String, &BindingExpression),
    mapping: &mut HashMap<ByAddress<ElementRc>, ElementRc>,
    root_component: &Rc<Component>,
) -> (String, BindingExpression) {
    let b = BindingExpression {
        expression: b.expression.clone(),
        span: b.span.clone(),
        priority: b.priority + 1,
        animation: b
            .animation
            .as_ref()
            .map(|pa| duplicate_property_animation(pa, mapping, root_component)),
        analysis: b.analysis.clone(),
        two_way_bindings: b.two_way_bindings.clone(),
    };
    (k.clone(), b)
}

fn duplicate_property_animation(
    v: &PropertyAnimation,
    mapping: &mut HashMap<ByAddress<ElementRc>, ElementRc>,
    root_component: &Rc<Component>,
) -> PropertyAnimation {
    match v {
        PropertyAnimation::Static(a) => {
            PropertyAnimation::Static(duplicate_element_with_mapping(a, mapping, root_component))
        }
        PropertyAnimation::Transition { state_ref, animations } => PropertyAnimation::Transition {
            state_ref: state_ref.clone(),
            animations: animations
                .iter()
                .map(|a| TransitionPropertyAnimation {
                    state_id: a.state_id,
                    is_out: a.is_out,
                    animation: duplicate_element_with_mapping(
                        &a.animation,
                        mapping,
                        root_component,
                    ),
                })
                .collect(),
        },
    }
}

fn fixup_reference(nr: &mut NamedReference, mapping: &HashMap<ByAddress<ElementRc>, ElementRc>) {
    if let Some(e) = mapping.get(&element_key(nr.element())) {
        *nr = NamedReference::new(e, nr.name());
    }
}

fn fixup_element_references(
    expr: &mut Expression,
    mapping: &HashMap<ByAddress<ElementRc>, ElementRc>,
) {
    if let Expression::ElementReference(element) = expr {
        if let Some(new_element) = element.upgrade().and_then(|e| mapping.get(&element_key(e))) {
            *element = Rc::downgrade(new_element);
        }
    } else {
        expr.visit_mut(|e| fixup_element_references(e, mapping));
    }
}

fn duplicate_transition(
    t: &Transition,
    mapping: &mut HashMap<ByAddress<ElementRc>, Rc<RefCell<Element>>>,
    root_component: &Rc<Component>,
) -> Transition {
    Transition {
        is_out: t.is_out,
        state_id: t.state_id.clone(),
        property_animations: t
            .property_animations
            .iter()
            .map(|(r, loc, anim)| {
                (
                    r.clone(),
                    loc.clone(),
                    duplicate_element_with_mapping(anim, mapping, root_component),
                )
            })
            .collect(),
        node: t.node.clone(),
    }
}

// Some components need to be inlined to avoid increased complexity in handling them
// in the code generators and subsequent passes.
fn component_requires_inlining(component: &Rc<Component>) -> bool {
    if component.child_insertion_point.borrow().is_some() {
        return true;
    }

    let root_element = &component.root_element;
    if super::flickable::is_flickable_element(root_element)
        || super::focus_item::get_explicit_forward_focus(root_element).is_some()
        || super::lower_layout::is_layout_element(root_element)
    {
        return true;
    }

    false
}
