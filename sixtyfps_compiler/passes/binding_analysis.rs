/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Compute binding analysis and attempt to find binding loops

use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::Expression;
use crate::langtype::Type;
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, ElementRc};

type PropertySet = linked_hash_set::LinkedHashSet<NamedReference>;

pub fn binding_analysis(component: &Rc<Component>, diag: &mut BuildDiagnostics) -> () {
    propagate_is_set_on_aliases(component);
    for g in component.used_global.borrow().iter() {
        propagate_is_set_on_aliases(g);
    }

    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |e, _| {
            for (name, binding) in &e.borrow().bindings {
                if matches!(e.borrow().lookup_property(name).property_type, Type::Callback { .. }) {
                    // TODO: We probably also want to do some analyzis on callbacks.
                    continue;
                }
                if binding.analysis.borrow().is_some() {
                    continue;
                }
                let mut set = PropertySet::default();
                analyse_binding(e, &name, &mut set, diag);
            }
        },
    );
}

fn analyse_binding(
    element: &ElementRc,
    name: &str,
    currently_analysing: &mut PropertySet,
    diag: &mut BuildDiagnostics,
) {
    let nr = NamedReference::new(element, name);
    if currently_analysing.back().map_or(false, |r| *r == nr)
        && matches!(element.borrow().bindings[name].expression, Expression::TwoWayBinding(..))
    {
        // This is already reported as an error by the remove_alias pass.
        // FIXME: maybe we should report it there instead
        return;
    }

    if currently_analysing.contains(&nr) {
        for p in currently_analysing.iter().rev() {
            if std::mem::replace(
                &mut p.element().borrow().bindings[p.name()]
                    .analysis
                    .borrow_mut()
                    .get_or_insert(Default::default())
                    .is_in_binding_loop,
                true,
            ) {
                break;
            }

            diag.push_error(
                format!("The binding for the property '{}' is part of a binding loop.", p.name()),
                &p.element().borrow().bindings[p.name()],
            );

            if *p == nr {
                break;
            }
        }
        return;
    }
    currently_analysing.insert(nr.clone());

    recurse_expression(&element.borrow().bindings[name], &mut |prop: &NamedReference| {
        if let Some(binding) = prop.element().borrow().bindings.get(prop.name()) {
            if binding.analysis.borrow().is_some() {
                return;
            }
            analyse_binding(&prop.element(), prop.name(), currently_analysing, diag);
        }
    });

    {
        let elem = element.borrow();
        let b = &elem.bindings[name];
        let is_const = b.expression.is_constant();

        let mut analysis = b.analysis.borrow_mut();
        let mut analysis = analysis.get_or_insert(Default::default());
        analysis.is_const = is_const;
    }

    let o = currently_analysing.pop_back();
    assert_eq!(o.unwrap(), nr);
}

// Same as in crate::visit_all_named_references_in_element, but not mut
fn recurse_expression(expr: &Expression, vis: &mut impl FnMut(&NamedReference)) {
    expr.visit(|sub| recurse_expression(sub, vis));
    match expr {
        Expression::PropertyReference(r) | Expression::CallbackReference(r) => vis(r),
        Expression::TwoWayBinding(r, _) => vis(r),
        Expression::LayoutCacheAccess { layout_cache_prop, .. } => vis(layout_cache_prop),
        Expression::SolveLayout(l) | Expression::ComputeLayoutInfo(l) => {
            let mut l = l.clone();
            if matches!(expr, Expression::ComputeLayoutInfo(_)) {
                // we should not visit the layout geometry in that case
                *l.rect_mut() = Default::default();
            }
            l.visit_named_references(&mut |nr| vis(nr));
        }
        _ => {}
    }
}

/// Make sure that the is_set property analasys is set to any property which has a two way binding
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
                if matches!(binding.expression, Expression::TwoWayBinding(..)) {
                    check_alias(e, name, &binding.expression);
                }
            }
        },
    );

    fn check_alias(e: &ElementRc, name: &str, binding: &Expression) -> () {
        let mut expr = Some(binding);
        while let Some(Expression::TwoWayBinding(_, next)) = expr {
            expr = next.as_ref().map(|e| &**e);
        }
        // Note: since the analysis hasn't been run, any property access will result in a non constant binding. this is slightly non-optimal
        let is_binding_constant = expr.map_or(true, |e| e.is_constant());
        if !is_binding_constant && !NamedReference::new(e, name).is_externaly_modified() {
            return;
        }

        propagate_alias(binding);
    }

    fn propagate_alias(binding: &Expression) {
        let mut expr = Some(binding);
        while let Some(Expression::TwoWayBinding(alias, next)) = expr {
            if !alias.is_externaly_modified() {
                let al_el = alias.element();
                let al_el = al_el.borrow();
                al_el
                    .property_analysis
                    .borrow_mut()
                    .entry(alias.name().into())
                    .or_default()
                    .is_set = true;
                if let Some(bind) = al_el.bindings.get(alias.name()) {
                    propagate_alias(bind)
                }
            }
            expr = next.as_ref().map(|e| &**e);
        }
    }
}
