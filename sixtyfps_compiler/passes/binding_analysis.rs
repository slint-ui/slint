/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Compute binding analysis and attempt to find binding loops

use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::diagnostics::Spanned;
use crate::expression_tree::BindingExpression;
use crate::expression_tree::BuiltinFunction;
use crate::expression_tree::Expression;
use crate::langtype::Type;
use crate::layout::LayoutItem;
use crate::layout::Orientation;
use crate::namedreference::NamedReference;
use crate::object_tree::Document;
use crate::object_tree::PropertyAnimation;
use crate::object_tree::{Component, ElementRc};

type PropertySet = linked_hash_set::LinkedHashSet<NamedReference>;

pub fn binding_analysis(doc: &Document, diag: &mut BuildDiagnostics) {
    let component = &doc.root_component;
    mark_used_base_properties(component);
    propagate_is_set_on_aliases(component);
    perform_binding_analysis(component, diag);
}

fn perform_binding_analysis(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |e, _| analyze_element(e, diag),
    );

    for c in &component.used_types.borrow().sub_components {
        perform_binding_analysis(c, diag);
    }
}

fn analyze_element(elem: &ElementRc, diag: &mut BuildDiagnostics) {
    for (name, binding) in &elem.borrow().bindings {
        if binding.borrow().analysis.is_some() {
            continue;
        }
        let mut set = PropertySet::default();
        analyse_binding(elem, name, &mut set, diag);
    }
}

fn analyse_binding(
    element: &ElementRc,
    name: &str,
    currently_analysing: &mut PropertySet,
    diag: &mut BuildDiagnostics,
) {
    let nr = NamedReference::new(element, name);
    if currently_analysing.back().map_or(false, |r| *r == nr)
        && !element.borrow().bindings[name].borrow().two_way_bindings.is_empty()
    {
        // This is already reported as an error by the remove_alias pass.
        // FIXME: maybe we should report it there instead
        return;
    }

    if currently_analysing.contains(&nr) {
        for p in currently_analysing.iter().rev() {
            let elem = p.element();
            let elem = elem.borrow();
            let binding = elem.bindings[p.name()].borrow();
            if binding.analysis.as_ref().unwrap().is_in_binding_loop.replace(true) {
                break;
            }

            let span =
                binding.span.clone().or_else(|| elem.node.as_ref().map(|n| n.to_source_location()));
            diag.push_error(
                format!("The binding for the property '{}' is part of a binding loop", p.name()),
                &span,
            );

            if *p == nr {
                break;
            }
        }
        return;
    }

    let binding = &element.borrow().bindings[name];
    if binding.borrow().analysis.is_some() {
        return;
    }
    binding.borrow_mut().analysis = Some(Default::default());
    currently_analysing.insert(nr.clone());

    let mut process_prop = |prop: &NamedReference| {
        let mut element = prop.element();
        element
            .borrow()
            .property_analysis
            .borrow_mut()
            .entry(prop.name().into())
            .or_default()
            .is_read = true;

        loop {
            if element.borrow().bindings.contains_key(prop.name()) {
                analyse_binding(&element, prop.name(), currently_analysing, diag);
            }
            let next = if let Type::Component(base) = &element.borrow().base_type {
                if element.borrow().property_declarations.contains_key(prop.name()) {
                    break;
                }
                base.root_element.clone()
            } else {
                break;
            };
            element = next;
            element
                .borrow()
                .property_analysis
                .borrow_mut()
                .entry(prop.name().into())
                .or_default()
                .is_read_externally = true;
        }
    };

    {
        let b = binding.borrow();
        for nr in &b.two_way_bindings {
            process_prop(nr);
        }
        recurse_expression(&b.expression, &mut process_prop);

        let mut is_const =
            b.expression.is_constant() && b.two_way_bindings.iter().all(|n| n.is_constant());

        if is_const && matches!(b.expression, Expression::Invalid) {
            // check the base
            if let Some(base) = element.borrow().sub_component() {
                is_const = NamedReference::new(&base.root_element, name).is_constant();
            }
        }
        drop(b);
        binding.borrow_mut().analysis.as_mut().unwrap().is_const = is_const;
    }

    match &binding.borrow().animation {
        Some(PropertyAnimation::Static(e)) => analyze_element(e, diag),
        Some(PropertyAnimation::Transition { animations, state_ref }) => {
            recurse_expression(state_ref, &mut process_prop);
            for a in animations {
                analyze_element(&a.animation, diag);
            }
        }
        None => (),
    }

    let o = currently_analysing.pop_back();
    assert_eq!(o.unwrap(), nr);
}

// Same as in crate::visit_all_named_references_in_element, but not mut
fn recurse_expression(expr: &Expression, vis: &mut impl FnMut(&NamedReference)) {
    expr.visit(|sub| recurse_expression(sub, vis));
    match expr {
        Expression::PropertyReference(r) | Expression::CallbackReference(r) => vis(r),
        Expression::LayoutCacheAccess { layout_cache_prop, .. } => vis(layout_cache_prop),
        Expression::SolveLayout(l, o) | Expression::ComputeLayoutInfo(l, o) => {
            // we should only visit the layout geometry for the orientation
            if matches!(expr, Expression::SolveLayout(..)) {
                l.rect().size_reference(*o).map(&mut |nr| vis(nr));
            }
            match l {
                crate::layout::Layout::GridLayout(l) => {
                    visit_layout_items_dependencies(l.elems.iter().map(|it| &it.item), *o, vis)
                }
                crate::layout::Layout::BoxLayout(l) => {
                    visit_layout_items_dependencies(l.elems.iter(), *o, vis)
                }
                crate::layout::Layout::PathLayout(l) => {
                    for it in &l.elements {
                        vis(&NamedReference::new(it, "width"));
                        vis(&NamedReference::new(it, "height"));
                    }
                }
            }
            if let Some(g) = l.geometry() {
                let mut g = g.clone();
                g.rect = Default::default(); // already visited;
                g.visit_named_references(&mut |nr| vis(nr))
            }
        }
        Expression::FunctionCall { function, arguments, .. } => {
            if let Expression::BuiltinFunctionReference(
                BuiltinFunction::ImplicitLayoutInfo(orientation),
                _,
            ) = &**function
            {
                if let [Expression::ElementReference(item)] = arguments.as_slice() {
                    visit_implicit_layout_info_dependencies(
                        *orientation,
                        &item.upgrade().unwrap(),
                        vis,
                    );
                }
            }
        }
        _ => {}
    }
}

fn visit_layout_items_dependencies<'a>(
    items: impl Iterator<Item = &'a LayoutItem>,
    orientation: Orientation,
    vis: &mut impl FnMut(&NamedReference),
) {
    for it in items {
        if let Some(nr) = it.element.borrow().layout_info_prop(orientation) {
            vis(nr);
        } else {
            if let Type::Component(base) = &it.element.borrow().base_type {
                if let Some(nr) = base.root_element.borrow().layout_info_prop(orientation) {
                    vis(nr);
                }
            }
            visit_implicit_layout_info_dependencies(orientation, &it.element, vis);
        }

        for (nr, _) in it.constraints.for_each_restrictions(orientation) {
            vis(nr)
        }
    }
}

/// The builtin function can call native code, and we need to visit the properties that are accessed by it
fn visit_implicit_layout_info_dependencies(
    orientation: crate::layout::Orientation,
    item: &ElementRc,
    vis: &mut impl FnMut(&NamedReference),
) {
    let base_type = item.borrow().base_type.to_string();
    match base_type.as_str() {
        "Image" => {
            vis(&NamedReference::new(item, "source"));
            if orientation == Orientation::Vertical {
                vis(&NamedReference::new(item, "width"));
            }
        }
        "Text" | "TextInput" => {
            vis(&NamedReference::new(item, "text"));
            vis(&NamedReference::new(item, "font-family"));
            vis(&NamedReference::new(item, "font-size"));
            vis(&NamedReference::new(item, "font-weight"));
            vis(&NamedReference::new(item, "letter-spacing"));
            vis(&NamedReference::new(item, "wrap"));
            if orientation == Orientation::Vertical {
                vis(&NamedReference::new(item, "width"));
            }
            if base_type.as_str() == "TextInput" {
                vis(&NamedReference::new(item, "single-line"));
            } else {
                vis(&NamedReference::new(item, "overflow"));
            }
        }

        _ => (),
    }
}

/// Make sure that the is_set property analysis is set to any property which has a two way binding
/// to a property that is, itself, is set
///
/// Example:
/// ```60
/// Xx := TouchArea {
///    property <int> bar <=> foo;
///    clicked => { bar+=1; }
///    property <int> foo; // must ensure that this is not considered as const, because the alias with bar
/// }
/// ```
fn propagate_is_set_on_aliases(component: &Rc<Component>) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |e, _| {
            for (name, binding) in &e.borrow().bindings {
                if !binding.borrow().two_way_bindings.is_empty() {
                    check_alias(e, name, &binding.borrow());
                }
            }
            for (_, decl) in &e.borrow().property_declarations {
                if let Some(alias) = &decl.is_alias {
                    mark_alias(alias)
                }
            }
        },
    );

    fn check_alias(e: &ElementRc, name: &str, binding: &BindingExpression) {
        // Note: since the analysis hasn't been run, any property access will result in a non constant binding. this is slightly non-optimal
        let is_binding_constant =
            binding.is_constant() && binding.two_way_bindings.iter().all(|n| n.is_constant());
        if is_binding_constant && !NamedReference::new(e, name).is_externally_modified() {
            return;
        }

        propagate_alias(binding);
    }

    fn propagate_alias(binding: &BindingExpression) {
        for alias in &binding.two_way_bindings {
            mark_alias(alias);
        }
    }

    fn mark_alias(alias: &NamedReference) {
        if !alias.is_externally_modified() {
            alias.mark_as_set();
            if let Some(bind) = alias.element().borrow().bindings.get(alias.name()) {
                propagate_alias(&bind.borrow())
            }
        }
    }

    for g in &component.used_types.borrow().globals {
        propagate_is_set_on_aliases(g);
    }
    for c in &component.used_types.borrow().sub_components {
        propagate_is_set_on_aliases(c);
    }
}

/// Make sure that the set_in_derived is true for all bindings
fn mark_used_base_properties(component: &Rc<Component>) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |element, _| {
            if !matches!(element.borrow().base_type, Type::Component(_)) {
                return;
            }
            for (name, binding) in &element.borrow().bindings {
                if binding.borrow().has_binding() {
                    crate::namedreference::mark_property_set_derived_in_base(element.clone(), name);
                }
            }
        },
    );

    for c in &component.used_types.borrow().sub_components {
        mark_used_base_properties(c);
    }
}
