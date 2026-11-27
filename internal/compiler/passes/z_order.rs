// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore zorder
/*! re-order the children by their z-order (static case) or mark elements
    for dynamic z-order sorting (when z is bound to a non-constant expression).
*/

use std::rc::Rc;

use crate::diagnostics::Spanned;
use crate::element_type::ElementType;
use crate::expression_tree::{BindingExpression, Expression};
use crate::expression_types::Unit;
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, ElementRc, ZOrder};

pub fn reorder_by_z_order(root_component: &Rc<Component>) {
    crate::object_tree::recurse_elem_including_sub_components(
        root_component,
        &(),
        &mut |elem: &ElementRc, _| {
            reorder_children_by_zorder(elem);
        },
    )
}

fn reorder_children_by_zorder(elem: &ElementRc) {
    let values: Vec<ZValue> = elem.borrow().children.iter().map(z_value).collect();

    if values.iter().all(|v| matches!(v, ZValue::Unset)) {
        return;
    }

    if values.iter().any(|v| matches!(v, ZValue::Dynamic)) {
        setup_dynamic_z_order(elem, &values);
    } else {
        reorder_static_z(elem, &values);
    }
}

/// What the compiler knows about the z of a child.
enum ZValue {
    /// No z: the default of 0.
    Unset,
    /// A z that can't change.
    /// `own_binding` is the element with the binding when the binding belongs to the child alone,
    /// so that the compile-time order can take it.
    Constant { z: f64, own_binding: Option<ElementRc> },
    /// A z that may change at runtime.
    Dynamic,
}

/// The z of a child: its own, or the one set on the root of the component it instantiates.
/// For a `for` or `if`, that's the root of the component created for it, and then of the
/// component that root instantiates, and so on.
///
/// A z that's linked, animated, or assigned at runtime is dynamic.
fn z_value(child_elm: &ElementRc) -> ZValue {
    let repeated = child_elm.borrow().repeated.is_some();

    let mut value = ZValue::Unset;
    let mut elem = child_elm.clone();
    let mut depth = 0;
    loop {
        {
            let e = elem.borrow();
            if e.property_analysis.borrow().get("z").is_some_and(|a| a.is_set || a.is_linked) {
                return ZValue::Dynamic;
            }
            // A binding closer to the child overrides the ones of its bases
            if matches!(value, ZValue::Unset)
                && let Some(b) = e.binding("z")
            {
                let fixed = b.two_way_bindings.is_empty() && b.animation.is_none();
                let Some(z) = try_eval_const_expr(&b.expression).filter(|_| fixed) else {
                    return ZValue::Dynamic;
                };
                let own = depth == 0 || (repeated && depth == 1);
                value = ZValue::Constant { z, own_binding: own.then(|| elem.clone()) };
            }
        }
        let base = match &elem.borrow().base_type {
            ElementType::Component(c) => c.root_element.clone(),
            _ => return value,
        };
        elem = base;
        depth += 1;
    }
}

/// The z of a child whose z can't change, taking the binding if it's the child's own.
fn take_constant_z(value: &ZValue) -> f64 {
    match value {
        ZValue::Constant { z, own_binding } => {
            if let Some(holder) = own_binding {
                holder.borrow_mut().take_binding("z");
            }
            *z
        }
        ZValue::Unset => 0.,
        ZValue::Dynamic => unreachable!("a dynamic z has no compile-time value"),
    }
}

/// Static z-order: evaluate all z values at compile time and reorder children.
fn reorder_static_z(elem: &ElementRc, values: &[ZValue]) {
    let mut children_z_order = Vec::new();
    for (idx, value) in values.iter().enumerate() {
        if matches!(value, ZValue::Constant { .. }) {
            if children_z_order.is_empty() {
                for i in 0..idx {
                    children_z_order.push((i, 0.));
                }
            }
            children_z_order.push((idx, take_constant_z(value)));
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
fn setup_dynamic_z_order(elem: &ElementRc, values: &[ZValue]) {
    for (child_elm, value) in elem.borrow().children.iter().zip(values) {
        let z_order = match value {
            ZValue::Dynamic if child_elm.borrow().repeated.is_some() => per_instance_z(child_elm),
            ZValue::Dynamic => dynamic_z(child_elm),
            // For a `for` or `if`, all instances share this z and stay together
            _ => ZOrder::Constant(take_constant_z(value) as f32),
        };
        child_elm.borrow_mut().z_order = Some(z_order);
    }
}

/// The z-order of a `for` or `if` with a dynamic z, which is read for each instance.
fn per_instance_z(child_elm: &ElementRc) -> ZOrder {
    let root = match &child_elm.borrow().base_type {
        ElementType::Component(c) => c.root_element.clone(),
        _ => unreachable!("a repeated element instantiates its repeated component"),
    };
    // The z property is read by the repeater at runtime; keep it materialized
    root.borrow()
        .property_analysis
        .borrow_mut()
        .entry(smol_str::SmolStr::new_static("z"))
        .or_default()
        .is_read = true;
    if root.borrow().binding("z").is_none() {
        crate::namedreference::mark_property_read_derived_in_base(root.clone(), "z");
    }
    ZOrder::PerInstance(NamedReference::new(&root, smol_str::SmolStr::new_static("z")))
}

/// The z-order of a child with a dynamic z, which is read on every visit.
fn dynamic_z(child_elm: &ElementRc) -> ZOrder {
    if child_elm.borrow().binding("z").is_none() {
        crate::namedreference::mark_property_read_derived_in_base(child_elm.clone(), "z");
    }
    // The z value is read at runtime; make sure a binding exists so that the
    // property is materialized.
    if child_elm.borrow().binding_cell_including_synthetic("z").is_none()
        && crate::layout::find_binding(child_elm, "z", |_, _, _| ()).is_none()
    {
        let span = child_elm.borrow().to_source_location();
        child_elm.borrow_mut().set_binding(
            smol_str::SmolStr::new_static("z"),
            BindingExpression::new_with_span(Expression::NumberLiteral(0., Unit::None), span),
        );
    }
    ZOrder::Dynamic(NamedReference::new(child_elm, smol_str::SmolStr::new_static("z")))
}

fn try_eval_const_expr(expression: &Expression) -> Option<f64> {
    match expression.ignore_debug_hooks() {
        Expression::NumberLiteral(v, Unit::None) => Some(*v),
        Expression::Cast { from, .. } => try_eval_const_expr(from),
        Expression::UnaryOp { sub, op: '-' } => try_eval_const_expr(sub).map(|v| -v),
        Expression::UnaryOp { sub, op: '+' } => try_eval_const_expr(sub),
        _ => None,
    }
}
