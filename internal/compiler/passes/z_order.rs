// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
    let children_count = elem.borrow().children.len();
    if children_count == 0 {
        return;
    }

    // First pass: determine if we have any z properties and whether they're all constant
    let mut has_any_z = false;
    let mut has_dynamic_z = false;

    for child_elm in elem.borrow().children.iter() {
        let has_z = child_elm.borrow().bindings.contains_key("z");
        let repeated_has_z = if !has_z {
            child_elm.borrow().repeated.is_some()
                && matches!(&child_elm.borrow().base_type, ElementType::Component(c)
                    if c.root_element.borrow().bindings.contains_key("z"))
        } else {
            false
        };

        if has_z || repeated_has_z {
            has_any_z = true;
            let is_const = if has_z {
                child_elm
                    .borrow()
                    .bindings
                    .get("z")
                    .map(|e| try_eval_const_expr(&e.borrow().expression).is_some())
                    .unwrap_or(false)
            } else if let ElementType::Component(c) = &child_elm.borrow().base_type {
                c.root_element
                    .borrow()
                    .bindings
                    .get("z")
                    .map(|e| try_eval_const_expr(&e.borrow().expression).is_some())
                    .unwrap_or(false)
            } else {
                false
            };
            if !is_const {
                has_dynamic_z = true;
            }
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

/// Dynamic z-order: mark the parent element and ensure all children have z bindings.
/// Non-repeater children get NamedReferences (materialized as runtime properties).
/// Repeater/conditional children get their z evaluated at compile time if constant,
/// or default to z=0.
fn setup_dynamic_z_order(
    elem: &Rc<RefCell<crate::object_tree::Element>>,
    diag: &mut BuildDiagnostics,
) {
    use crate::namedreference::NamedReference;
    use crate::object_tree::ZOrder;

    elem.borrow_mut().has_dynamic_z_order = true;

    for child_elm in elem.borrow().children.iter() {
        if child_elm.borrow().repeated.is_some() {
            // Repeater/conditional child: z lives in the inner component.
            // Must be a compile-time constant; per-item dynamic z in repeaters
            // is not yet supported.
            let z_val = if let ElementType::Component(c) = &child_elm.borrow().base_type {
                let binding = c.root_element.borrow_mut().bindings.remove("z");
                if let Some(e) = binding {
                    if let Some(val) = try_eval_const_expr(&e.borrow().expression) {
                        val
                    } else {
                        diag.push_error(
                            "'z' in a repeated element must be a constant".into(),
                            &*e.borrow(),
                        );
                        0.
                    }
                } else {
                    0.
                }
            } else {
                0.
            };
            child_elm.borrow_mut().z_order = Some(ZOrder::Constant(z_val as f32));
        } else {
            // Non-repeater child: create NamedReference for runtime access.
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
            let nr = NamedReference::new(child_elm, smol_str::SmolStr::new_static("z"));
            child_elm.borrow_mut().z_order = Some(ZOrder::Dynamic(nr));
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
