// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass creates properties that are used but are otherwise not real.
//!
//! Must be run after lower_layout and default_geometry passes

use crate::diagnostics::Spanned;
use crate::expression_tree::{BindingExpression, Expression, Unit};
use crate::langtype::{ElementType, Type};
use crate::layout::Orientation;
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

pub fn materialize_fake_properties(component: &Rc<Component>) {
    let mut to_materialize = std::collections::HashMap::new();

    visit_all_named_references(component, &mut |nr| {
        let elem_rc = nr.element();
        if !to_materialize.contains_key(nr)
            && let Some(ty) = should_materialize(&elem_rc, nr.name())
        {
            let elem = elem_rc.borrow();
            // This only brings more trouble down the line
            if elem.repeated.is_some() {
                panic!(
                    "Cannot materialize fake property {} on repeated element {}",
                    nr.name(),
                    elem.id
                );
            }
            to_materialize.insert(nr.clone(), ty);
        }
    });

    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        for prop in elem.borrow().bindings.keys() {
            let nr = NamedReference::new(elem, prop.clone());
            if let std::collections::hash_map::Entry::Vacant(e) = to_materialize.entry(nr)
                && let Some(ty) = should_materialize(elem, prop)
            {
                e.insert(ty);
            }
        }
    });

    for (nr, ty) in to_materialize {
        let elem = nr.element();

        elem.borrow_mut().property_declarations.insert(
            nr.name().clone(),
            PropertyDeclaration { property_type: ty, ..PropertyDeclaration::default() },
        );

        if !must_initialize(&elem.borrow(), nr.name()) {
            // One must check again if one really need to be initialized, because when
            // we checked the first time, the element's binding were temporarily moved
            // by visit_all_named_references_in_element
            continue;
        }
        if let Some(init_expr) = initialize(&elem, nr.name()) {
            let mut elem_mut = elem.borrow_mut();
            let span = elem_mut.to_source_location();
            match elem_mut.bindings.entry(nr.name().clone()) {
                std::collections::btree_map::Entry::Vacant(e) => {
                    let mut binding = BindingExpression::new_with_span(init_expr, span);
                    binding.priority = i32::MAX;
                    e.insert(binding.into());
                }
                std::collections::btree_map::Entry::Occupied(mut e) => {
                    e.get_mut().get_mut().expression = init_expr;
                }
            }
        }
    }
}

// One must initialize if there is an actual expression for that binding
fn must_initialize(elem: &Element, prop: &str) -> bool {
    match elem.bindings.get(prop) {
        None => true,
        Some(b) => matches!(b.borrow().expression, Expression::Invalid),
    }
}

/// Returns a type if the property needs to be materialized.
/// It should be materialized for example if
/// - it is not a reserved property
/// - it is not a property of the base type
fn should_materialize(element_rc: &Rc<RefCell<Element>>, prop: &str) -> Option<Type> {
    let element = element_rc.borrow();

    if has_declared_property(&element, prop) {
        return None;
    }

    let ty = crate::typeregister::reserved_property(Cow::Borrowed(prop)).property_type.clone();
    if ty != Type::Invalid {
        Some(ty)
    } else if prop == "close-on-click" {
        // PopupWindow::close-on-click
        Some(Type::Bool)
    } else if prop == "close-policy" {
        // PopupWindow::close-policy
        Some(Type::Enumeration(
            crate::typeregister::BUILTIN.with(|e| e.enums.PopupClosePolicy.clone()),
        ))
    } else {
        let ty = element.base_type.lookup_property(prop).property_type.clone();
        (ty != Type::Invalid).then_some(ty)
    }
}

/// Returns true if the property is declared in this element or parent
/// (as opposed to being implicitly declared)
pub fn has_declared_property(elem: &Element, prop: &str) -> bool {
    if prop == "anchor"
        && elem.enclosing_component.upgrade().is_some_and(|c| c.inherits_popup_window.get())
    {
        return true;
    }
    if elem.property_declarations.contains_key(prop) {
        return true;
    }
    match &elem.base_type {
        ElementType::Component(c) => has_declared_property(&c.root_element.borrow(), prop),
        ElementType::Builtin(b) => b.native_class.lookup_property(prop).is_some(),
        ElementType::Native(n) => n.lookup_property(prop).is_some(),
        ElementType::Global | ElementType::Interface | ElementType::Error => false,
    }
}

/// Initialize a sensible default binding for the now materialized property
pub fn initialize(elem: &ElementRc, name: &str) -> Option<Expression> {
    let mut base_type = elem.borrow().base_type.clone();
    loop {
        base_type = match base_type {
            ElementType::Component(ref c) => c.root_element.borrow().base_type.clone(),
            ElementType::Builtin(b) => {
                match b.properties.get(name).and_then(|prop| prop.default_value.expr(elem)) {
                    Some(expr) => return Some(expr),
                    None => break,
                }
            }
            _ => break,
        };
    }

    // Hardcode properties for images, because this is a very common call, and this allows
    // later optimization steps to eliminate these properties.
    // Note that Rectangles and Empties are similarly optimized in layout_constraint_prop, and
    // we rely on struct field access simplification for those.
    if elem.borrow().builtin_type().is_some_and(|n| n.name == "Image") {
        if elem.borrow().layout_info_prop(Orientation::Horizontal).is_none() {
            match name {
                "min-width" => return Some(Expression::NumberLiteral(0., Unit::Px)),
                "max-width" => return Some(Expression::NumberLiteral(f32::MAX as _, Unit::Px)),
                "horizontal-stretch" => return Some(Expression::NumberLiteral(0., Unit::None)),
                _ => {}
            }
        }

        if elem.borrow().layout_info_prop(Orientation::Vertical).is_none() {
            match name {
                "min-height" => return Some(Expression::NumberLiteral(0., Unit::Px)),
                "max-height" => return Some(Expression::NumberLiteral(f32::MAX as _, Unit::Px)),
                "vertical-stretch" => return Some(Expression::NumberLiteral(0., Unit::None)),
                _ => {}
            }
        }
    }

    let expr = match name {
        "min-height" => layout_constraint_prop(elem, "min", Orientation::Vertical),
        "min-width" => layout_constraint_prop(elem, "min", Orientation::Horizontal),
        "max-height" => layout_constraint_prop(elem, "max", Orientation::Vertical),
        "max-width" => layout_constraint_prop(elem, "max", Orientation::Horizontal),
        "horizontal-stretch" => layout_constraint_prop(elem, "stretch", Orientation::Horizontal),
        "vertical-stretch" => layout_constraint_prop(elem, "stretch", Orientation::Vertical),
        "preferred-height" => layout_constraint_prop(elem, "preferred", Orientation::Vertical),
        "preferred-width" => layout_constraint_prop(elem, "preferred", Orientation::Horizontal),
        "opacity" => Expression::NumberLiteral(1., Unit::None),
        "visible" => Expression::BoolLiteral(true),
        "rowspan" => Expression::NumberLiteral(1., Unit::None),
        "colspan" => Expression::NumberLiteral(1., Unit::None),
        "rotation-origin-x" => size_div_2(elem, "width"),
        "rotation-origin-y" => size_div_2(elem, "height"),
        _ => return None,
    };
    Some(expr)
}

fn layout_constraint_prop(elem: &ElementRc, field: &str, orient: Orientation) -> Expression {
    let expr = match elem.borrow().layout_info_prop(orient) {
        Some(e) => Expression::PropertyReference(e.clone()),
        None => crate::layout::implicit_layout_info_call(
            elem,
            orient,
            crate::layout::BuiltinFilter::All,
            None,
        )
        .unwrap(),
    };
    Expression::StructFieldAccess { base: expr.into(), name: field.into() }
}

fn size_div_2(elem: &ElementRc, field: &str) -> Expression {
    Expression::BinaryExpression {
        lhs: Expression::PropertyReference(NamedReference::new(elem, field.into())).into(),
        op: '/',
        rhs: Expression::NumberLiteral(2., Unit::None).into(),
    }
}
