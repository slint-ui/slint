// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

//! Inline each object_tree::Component within the main Component

use crate::expression_tree::{BindingExpression, Expression, NamedReference};
use crate::langtype::{ElementType, Type};
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
            if let ElementType::Component(c) = base {
                // First, make sure that the component itself is properly inlined
                inline_components_recursively(&c, inline_selection);

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
                            // We always inline the root in case the element that instantiate this component needs full inlining
                            || Rc::ptr_eq(elem, &component.root_element)
                    }
                } {
                    inline_element(elem, &c, component);
                }
            }
        });
        component
            .popup_windows
            .borrow()
            .iter()
            .for_each(|p| inline_components_recursively(&p.component, inline_selection))
    }
    inline_components_recursively(&doc.root_component, inline_selection);

    if matches!(inline_selection, InlineSelection::InlineAllComponents) {
        let mut init_code = doc.root_component.init_code.borrow_mut();
        let inlined_init_code = core::mem::take(&mut init_code.inlined_init_code);
        init_code.constructor_code.splice(0..0, inlined_init_code.into_values());
    }
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
        inlined_component.root_element.borrow().property_declarations.iter().map(clone_tuple),
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

    let mut new_children = vec![];
    new_children
        .reserve(elem_mut.children.len() + inlined_component.root_element.borrow().children.len());
    new_children.extend(
        inlined_component.root_element.borrow().children.iter().map(|x| {
            duplicate_element_with_mapping(x, &mut mapping, root_component, priority_delta)
        }),
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

    if let ElementType::Component(c) = &mut elem_mut.base_type {
        if c.parent_element.upgrade().is_some() {
            debug_assert!(Rc::ptr_eq(elem, &c.parent_element.upgrade().unwrap()));
            *c = duplicate_sub_component(c, elem, &mut mapping, priority_delta);
        }
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

    if let Some(orig) = &inlined_component.root_element.borrow().layout_info_prop {
        if let Some(_new) = &mut elem_mut.layout_info_prop {
            todo!("Merge layout infos");
        } else {
            elem_mut.layout_info_prop = Some(orig.clone());
        }
    }

    core::mem::drop(elem_mut);

    let inlined_init_code = inlined_component
        .init_code
        .borrow()
        .inlined_init_code
        .values()
        .cloned()
        .chain(inlined_component.init_code.borrow().constructor_code.iter().map(
            |constructor_code_expr| {
                // Fix up any property references from within already collected init code.
                let mut new_constructor_code = constructor_code_expr.clone();
                visit_named_references_in_expression(&mut new_constructor_code, &mut |nr| {
                    fixup_reference(nr, &mapping)
                });
                fixup_element_references(&mut new_constructor_code, &mapping);
                new_constructor_code
            },
        ))
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
}

// Duplicate the element elem and all its children. And fill the mapping to point from the old to the new
fn duplicate_element_with_mapping(
    element: &ElementRc,
    mapping: &mut HashMap<ByAddress<ElementRc>, ElementRc>,
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
        property_analysis: elem.property_analysis.clone(),
        children: elem
            .children
            .iter()
            .map(|x| duplicate_element_with_mapping(x, mapping, root_component, priority_delta))
            .collect(),
        repeated: elem.repeated.clone(),
        node: elem.node.clone(),
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
        named_references: Default::default(),
        item_index: Default::default(), // Not determined yet
        item_index_of_first_children: Default::default(),
        is_flickable_viewport: elem.is_flickable_viewport,
        has_popup_child: elem.has_popup_child,
        is_legacy_syntax: elem.is_legacy_syntax,
        inline_depth: elem.inline_depth + 1,
    }));
    mapping.insert(element_key(element.clone()), new.clone());
    if let ElementType::Component(c) = &mut new.borrow_mut().base_type {
        if c.parent_element.upgrade().is_some() {
            debug_assert!(Rc::ptr_eq(element, &c.parent_element.upgrade().unwrap()));
            *c = duplicate_sub_component(c, &new, mapping, priority_delta);
        }
    };

    new
}

/// Duplicate Component for repeated element or popup window that have a parent_element
fn duplicate_sub_component(
    component_to_duplicate: &Rc<Component>,
    new_parent: &ElementRc,
    mapping: &mut HashMap<ByAddress<ElementRc>, ElementRc>,
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
        embedded_file_resources: component_to_duplicate.embedded_file_resources.clone(),
        root_constraints: component_to_duplicate.root_constraints.clone(),
        child_insertion_point: component_to_duplicate.child_insertion_point.clone(),
        init_code: component_to_duplicate.init_code.clone(),
        used_types: Default::default(),
        popup_windows: Default::default(),
        exported_global_names: component_to_duplicate.exported_global_names.clone(),
        is_root_component: Default::default(),
        private_properties: Default::default(),
    };

    let new_component = Rc::new(new_component);
    let weak = Rc::downgrade(&new_component);
    recurse_elem(&new_component.root_element, &(), &mut |e, _| {
        e.borrow_mut().enclosing_component = weak.clone()
    });
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
    new_component
        .root_constraints
        .borrow_mut()
        .visit_named_references(&mut |nr| fixup_reference(nr, mapping));
    new_component
}

fn duplicate_popup(
    p: &PopupWindow,
    mapping: &mut HashMap<ByAddress<ElementRc>, ElementRc>,
    priority_delta: i32,
) -> PopupWindow {
    let parent = mapping
        .get(&element_key(p.component.parent_element.upgrade().expect("must have a parent")))
        .expect("Parent must be in the mapping")
        .clone();
    PopupWindow {
        x: p.x.clone(),
        y: p.y.clone(),
        close_on_click: p.close_on_click.clone(),
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
    (k, b): (&String, &RefCell<BindingExpression>),
    mapping: &mut HashMap<ByAddress<ElementRc>, ElementRc>,
    root_component: &Rc<Component>,
    priority_delta: i32,
) -> (String, RefCell<BindingExpression>) {
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
    mapping: &mut HashMap<ByAddress<ElementRc>, ElementRc>,
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
                    is_out: a.is_out,
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

fn fixup_reference(nr: &mut NamedReference, mapping: &HashMap<ByAddress<ElementRc>, ElementRc>) {
    if let Some(e) = mapping.get(&element_key(nr.element())) {
        *nr = NamedReference::new(e, nr.name());
    }
}

fn fixup_element_references(
    expr: &mut Expression,
    mapping: &HashMap<ByAddress<ElementRc>, ElementRc>,
) {
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
        Expression::SolveLayout(l, _) | Expression::ComputeLayoutInfo(l, _) => match l {
            crate::layout::Layout::GridLayout(l) => {
                for e in &mut l.elems {
                    fxe(&mut e.item.element);
                }
            }
            crate::layout::Layout::BoxLayout(l) => {
                for e in &mut l.elems {
                    fxe(&mut e.element);
                }
            }
        },
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
        is_out: t.is_out,
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

    // the focus_item pass needs to refer to elements that are focusable, if it is not inline
    // it is not possible to refer to them in an  Expression::ElementReference
    if matches!(&root_element.borrow().base_type, ElementType::Builtin(b) if b.accepts_focus) {
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
            // If there is an animation, we currently inline so that if this property
            // is set with a binding, it is merged
            return true;
        }
    }

    false
}

fn element_require_inlining(elem: &ElementRc) -> bool {
    if !elem.borrow().children.is_empty() {
        // the generators assume that the children list is complete, which sub-components may break
        return true;
    }
    for (prop, binding) in &elem.borrow().bindings {
        if prop == "clip" {
            // otherwise the children of the clipped items won't get moved as child of the Clip element
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
