// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore zorder
/*! re-order the children by their z-order (static case) or mark elements
    for dynamic z-order sorting (when z is bound to a non-constant expression).
*/

use std::cell::RefCell;
use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BindingExpression, Expression, Unit};
use crate::langtype::ElementType;
use crate::object_tree::{Component, ElementRc};

pub fn reorder_by_z_order(root_component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components(
        root_component,
        &(),
        &mut |elem: &ElementRc, _| {
            reorder_children_by_zorder(elem, diag);
        },
    )
}

fn reorder_children_by_zorder(
    elem: &Rc<RefCell<crate::object_tree::Element>>,
    diag: &mut BuildDiagnostics,
) {
    let mut has_any_z = false;
    let mut has_dynamic_z = false;

    for child_elm in elem.borrow().children.iter() {
        if mark_per_instance_z(child_elm) {
            has_any_z = true;
            has_dynamic_z = true;
        } else if let Some(value) = z_binding_value(child_elm) {
            has_any_z = true;
            has_dynamic_z |= value.is_none();
        }
    }

    if !has_any_z {
        return;
    }

    if has_dynamic_z {
        setup_dynamic_z_order(elem, diag);
    } else {
        reorder_static_z(elem, diag);
    }
}

/// If the child is a repeated element (`for` or `if`) with a non-constant z binding,
/// mark it so that its instances are expanded and sorted individually among the
/// parent's children, and return true.
fn mark_per_instance_z(child_elm: &ElementRc) -> bool {
    use crate::namedreference::NamedReference;

    let child = child_elm.borrow();
    if child.repeated.is_none() {
        return false;
    }
    let ElementType::Component(c) = &child.base_type else { return false };
    let c = c.clone();
    drop(child);

    {
        let root = c.root_element.borrow();
        let Some(b) = root.bindings.get("z") else { return false };
        if try_eval_const_expr(&b.borrow().expression).is_some() {
            // Constant z: the whole repeater is ordered among its siblings
            return false;
        }
    }

    child_elm.borrow_mut().z_order = Some(crate::object_tree::ZOrder::PerInstance(
        NamedReference::new(&c.root_element, smol_str::SmolStr::new_static("z")),
    ));
    // The z property is read by the repeater at runtime; keep it materialized
    c.root_element
        .borrow()
        .property_analysis
        .borrow_mut()
        .entry(smol_str::SmolStr::new_static("z"))
        .or_default()
        .is_read = true;
    true
}

/// Static z-order: evaluate all z values at compile time and reorder children.
fn reorder_static_z(elem: &Rc<RefCell<crate::object_tree::Element>>, diag: &mut BuildDiagnostics) {
    let mut children_z_order = Vec::new();
    for (idx, child_elm) in elem.borrow().children.iter().enumerate() {
        let z = child_elm
            .borrow_mut()
            .bindings
            .remove("z")
            .and_then(|e| eval_const_expr(&e.borrow().expression, "z", &*e.borrow(), diag));
        let z =
            z.or_else(|| {
                child_elm.borrow().repeated.as_ref()?;
                if let ElementType::Component(c) = &child_elm.borrow().base_type {
                    c.root_element.borrow_mut().bindings.remove("z").and_then(|e| {
                        eval_const_expr(&e.borrow().expression, "z", &*e.borrow(), diag)
                    })
                } else {
                    None
                }
            });

        if let Some(z) = z {
            if children_z_order.is_empty() {
                for i in 0..idx {
                    children_z_order.push((i, 0.));
                }
            }
            children_z_order.push((idx, z));
        } else if !children_z_order.is_empty() {
            children_z_order.push((idx, 0.));
        }
    }

    if !children_z_order.is_empty() {
        children_z_order.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());

        let new_children = children_z_order
            .into_iter()
            .map(|(idx, _)| elem.borrow().children[idx].clone())
            .collect();
        elem.borrow_mut().children = new_children;
    }
}

/// Dynamic z-order: set the `z_order` of every child. Children whose z is a runtime
/// value get a NamedReference to their z property (materialized as a runtime property),
/// the other children a compile-time constant.
fn setup_dynamic_z_order(
    elem: &Rc<RefCell<crate::object_tree::Element>>,
    diag: &mut BuildDiagnostics,
) {
    use crate::namedreference::NamedReference;
    use crate::object_tree::ZOrder;

    for child_elm in elem.borrow().children.iter() {
        if child_elm.borrow().z_order.is_some() {
            // A repeated element with per-instance z, already set up by
            // `mark_per_instance_z`
            continue;
        }
        let z_order = if child_elm.borrow().repeated.is_some() {
            // Repeater/conditional child with a constant z (a non-constant z is
            // handled per instance by `mark_per_instance_z`): the whole repeater
            // is ordered among its siblings.
            let mut z_val = 0.;
            if let ElementType::Component(c) = &child_elm.borrow().base_type {
                let binding = c.root_element.borrow_mut().bindings.remove("z");
                if let Some(e) = binding {
                    z_val = try_eval_const_expr(&e.borrow().expression).unwrap_or_else(|| {
                        diag.push_error(
                            "'z' in a repeated element must be a constant".into(),
                            &*e.borrow(),
                        );
                        0.
                    });
                }
            }
            ZOrder::Constant(z_val as f32)
        } else if let Some(z_val) = constant_z(child_elm) {
            child_elm.borrow_mut().bindings.remove("z");
            ZOrder::Constant(z_val as f32)
        } else {
            // The z value is read at runtime; make sure a binding exists so that the
            // property is materialized.
            if !child_elm.borrow().bindings.contains_key("z") {
                let span = child_elm.borrow().to_source_location();
                child_elm.borrow_mut().bindings.insert(
                    smol_str::SmolStr::new_static("z"),
                    BindingExpression::new_with_span(
                        Expression::NumberLiteral(0., Unit::None),
                        span,
                    )
                    .into(),
                );
            }
            ZOrder::Dynamic(NamedReference::new(child_elm, smol_str::SmolStr::new_static("z")))
        };
        child_elm.borrow_mut().z_order = Some(z_order);
    }
}

/// The z binding of a child element, also looking into the root of repeated components:
/// `None` if there is no z binding, `Some(None)` if the value is not a constant expression,
/// `Some(Some(v))` for a constant.
fn z_binding_value(child_elm: &ElementRc) -> Option<Option<f64>> {
    let child = child_elm.borrow();
    if let Some(b) = child.bindings.get("z") {
        return Some(try_eval_const_expr(&b.borrow().expression));
    }
    if child.repeated.is_some()
        && let ElementType::Component(c) = &child.base_type
        && let Some(b) = c.root_element.borrow().bindings.get("z")
    {
        return Some(try_eval_const_expr(&b.borrow().expression));
    }
    None
}

/// The z value of a non-repeated child if it is known at compile time and cannot
/// change at runtime (no z binding at all means the default of 0)
fn constant_z(child_elm: &ElementRc) -> Option<f64> {
    let child = child_elm.borrow();
    if child.property_analysis.borrow().get("z").is_some_and(|a| a.is_set || a.is_linked) {
        return None;
    }
    match child.bindings.get("z") {
        None => Some(0.),
        Some(b) => {
            let b = b.borrow();
            if b.two_way_bindings.is_empty() && b.animation.is_none() {
                try_eval_const_expr(&b.expression)
            } else {
                None
            }
        }
    }
}

fn try_eval_const_expr(expression: &Expression) -> Option<f64> {
    match super::ignore_debug_hooks(expression) {
        Expression::NumberLiteral(v, Unit::None) => Some(*v),
        Expression::Cast { from, .. } => try_eval_const_expr(from),
        Expression::UnaryOp { sub, op: '-' } => try_eval_const_expr(sub).map(|v| -v),
        Expression::UnaryOp { sub, op: '+' } => try_eval_const_expr(sub),
        _ => None,
    }
}

fn eval_const_expr(
    expression: &Expression,
    name: &str,
    span: &dyn crate::diagnostics::Spanned,
    diag: &mut BuildDiagnostics,
) -> Option<f64> {
    let result = try_eval_const_expr(expression);
    if result.is_none() {
        diag.push_error(format!("'{name}' must be an number literal"), span);
    }
    result
}
