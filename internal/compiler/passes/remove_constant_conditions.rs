// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Remove `if false` conditional elements
//!
//! Conditions often fold to a literal only during const propagation, long
//! after the repeater machinery for the dead subtree was built; this pass
//! deletes it again.

use crate::expression_tree::Expression;
use crate::layout::{BOX_LAYOUT_CACHE_ENTRIES_PER_CELL, Layout};
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use std::collections::HashMap;
use std::rc::Rc;

pub fn remove_constant_conditions(component: &Rc<Component>) {
    let mut dead = Vec::new();
    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        let e = elem.borrow();
        // Grid cells keep their per-child repeater; the ComponentContainer placeholder is a
        // permanent false repeater that must survive as the embed slot.
        if e.repeated.as_ref().is_some_and(|r| {
            r.is_conditional_element && matches!(r.model, Expression::BoolLiteral(false))
        }) && e.grid_layout_cell.is_none()
            && !e.is_component_placeholder
        {
            dead.push(elem.clone());
        }
    });
    if dead.is_empty() {
        return;
    }

    // Flexbox cells keep their length-zero repeater; the measure path queries
    // cells individually.
    visit_all_expressions(component, |expr, _| {
        expr.visit_recursive_mut(&mut |e| {
            if let Expression::SolveFlexboxLayout(l)
            | Expression::ComputeFlexboxLayoutInfo { layout: l, .. } = e
            {
                dead.retain(|c| !l.elems.iter().any(|it| Rc::ptr_eq(&it.item.element, c)));
            }
        })
    });
    if dead.is_empty() {
        return;
    }
    let is_dead = |e: &ElementRc| dead.iter().any(|d| Rc::ptr_eq(d, e));

    // The component root of each removed conditional. Its box cell's constraints reference this
    // root, so a stale debug snapshot below is left still naming it.
    let mut dead_roots = std::collections::HashSet::new();
    for c in &dead {
        if let crate::langtype::ElementType::Component(base) = &c.borrow().base_type {
            dead_roots.insert(Rc::as_ptr(&base.root_element));
        }
    }

    let mut fixes: HashMap<NamedReference, Vec<usize>> = HashMap::new();
    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        visit_element_expressions(elem, |expr, name, _| {
            let Some(name) = name else { return };
            expr.visit_recursive_mut(&mut |e| match e {
                Expression::SolveBoxLayout(l, _) => {
                    let bases: Vec<usize> = l
                        .elems
                        .iter()
                        .enumerate()
                        .filter(|(_, it)| is_dead(&it.element))
                        .map(|(k, _)| BOX_LAYOUT_CACHE_ENTRIES_PER_CELL * k)
                        .collect();
                    if !bases.is_empty() {
                        fixes.insert(NamedReference::new(elem, name.into()), bases);
                        l.elems.retain(|it| !is_dead(&it.element));
                    }
                }
                Expression::ComputeBoxLayoutInfo { layout: l, .. } => {
                    l.elems.retain(|it| !is_dead(&it.element))
                }
                _ => {}
            });
        });
        // Drop debug cells naming a removed root, or a NamedReference to a dropped element crashes
        // a later pass; the layout-expression removal above does not reach these snapshots.
        for d in elem.borrow_mut().debug.iter_mut() {
            if let Some(Layout::BoxLayout(l)) = d.layout.as_mut() {
                l.elems.retain(|it| {
                    let mut hit = false;
                    it.constraints.clone().visit_named_references(&mut |nr| {
                        hit |= dead_roots.contains(&Rc::as_ptr(&nr.element()))
                    });
                    !hit
                });
            }
        }
        elem.borrow_mut().children.retain(|c| !is_dead(c));
    });

    // Each removed cell freed BOX_LAYOUT_CACHE_ENTRIES_PER_CELL cache slots whose indices were
    // baked into geometry bindings, so shift every surviving access behind it down.
    visit_all_expressions(component, |expr, _| {
        expr.visit_recursive_mut(&mut |e| {
            if let Expression::LayoutCacheAccess { layout_cache_prop, index, .. } = e
                && let Some(bases) = fixes.get(layout_cache_prop)
            {
                *index -= BOX_LAYOUT_CACHE_ENTRIES_PER_CELL
                    * bases.iter().filter(|b| **b < *index).count();
            }
        })
    });
}

#[test]
fn removes_constant_false_conditionals() {
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.style = Some("fluent".into());
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
export component Foo {
    in property <bool> dynamic;
    property <bool> never: false;
    if false: Rectangle {}
    if never: Rectangle {}
    if dynamic: Rectangle {}
}
"#
        .into(),
        Some(std::path::Path::new("test.slint")),
        &mut test_diags,
    );
    let (doc, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors(), "slint compile error {:#?}", diag.to_string_vec());

    let foo = doc.inner_components.iter().find(|c| c.id == "Foo").unwrap();
    let mut models = Vec::new();
    recurse_elem_including_sub_components(foo, &(), &mut |elem, _| {
        if let Some(r) = &elem.borrow().repeated
            && r.is_conditional_element
        {
            models.push(r.model.clone());
        }
    });

    // `never` folds to false during const propagation and is removed like the literal
    // `if false`; only `dynamic`, an `in` property, stays non-constant and survives, its
    // model still a runtime expression rather than a folded literal.
    assert_eq!(models.len(), 1, "{models:?}");
    assert!(!matches!(models[0], Expression::BoolLiteral(_)), "{:?}", models[0]);
}
