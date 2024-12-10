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
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::rc::Rc;

pub fn materialize_fake_properties(component: &Rc<Component>) {
    let mut to_materialize = std::collections::HashMap::new();

    visit_all_named_references(component, &mut |nr| {
        let elem = nr.element();
        let elem = elem.borrow();
        if !to_materialize.contains_key(nr) {
            if let Some(ty) =
                should_materialize(&elem.property_declarations, &elem.base_type, nr.name())
            {
                to_materialize.insert(nr.clone(), ty);
            }
        }
    });

    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        for prop in elem.borrow().bindings.keys() {
            let nr = NamedReference::new(elem, prop.clone());
            if let std::collections::hash_map::Entry::Vacant(e) = to_materialize.entry(nr) {
                let elem = elem.borrow();
                if let Some(ty) =
                    should_materialize(&elem.property_declarations, &elem.base_type, prop)
                {
                    e.insert(ty);
                }
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
fn should_materialize(
    property_declarations: &BTreeMap<SmolStr, PropertyDeclaration>,
    base_type: &ElementType,
    prop: &str,
) -> Option<Type> {
    if property_declarations.contains_key(prop) {
        return None;
    }
    let has_declared_property = match base_type {
        ElementType::Component(c) => has_declared_property(&c.root_element.borrow(), prop),
        ElementType::Builtin(b) => b.native_class.lookup_property(prop).is_some(),
        ElementType::Native(n) => n.lookup_property(prop).is_some(),
        ElementType::Global | ElementType::Error => false,
    };

    if !has_declared_property {
        let ty = crate::typeregister::reserved_property(prop).property_type;
        if ty != Type::Invalid {
            return Some(ty);
        } else if prop == "close-on-click" {
            // PopupWindow::close-on-click
            return Some(Type::Bool);
        } else if prop == "close-policy" {
            // PopupWindow::close-policy
            return Some(Type::Enumeration(
                crate::typeregister::BUILTIN.with(|e| e.enums.PopupClosePolicy.clone()),
            ));
        } else {
            let ty = base_type.lookup_property(prop).property_type.clone();
            return (ty != Type::Invalid).then_some(ty);
        }
    }
    None
}

/// Returns true if the property is declared in this element or parent
/// (as opposed to being implicitly declared)
fn has_declared_property(elem: &Element, prop: &str) -> bool {
    if elem.property_declarations.contains_key(prop) {
        return true;
    }
    match &elem.base_type {
        ElementType::Component(c) => has_declared_property(&c.root_element.borrow(), prop),
        ElementType::Builtin(b) => b.native_class.lookup_property(prop).is_some(),
        ElementType::Native(n) => n.lookup_property(prop).is_some(),
        ElementType::Global | ElementType::Error => false,
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

    let expr = match name {
        "min-height" => layout_constraint_prop(elem, "min", Orientation::Vertical),
        "min-width" => layout_constraint_prop(elem, "min", Orientation::Horizontal),
        "max-height" => layout_constraint_prop(elem, "max", Orientation::Vertical),
        "max-width" => layout_constraint_prop(elem, "max", Orientation::Horizontal),
        "preferred-height" => layout_constraint_prop(elem, "preferred", Orientation::Vertical),
        "preferred-width" => layout_constraint_prop(elem, "preferred", Orientation::Horizontal),
        "horizontal-stretch" => layout_constraint_prop(elem, "stretch", Orientation::Horizontal),
        "vertical-stretch" => layout_constraint_prop(elem, "stretch", Orientation::Vertical),
        "opacity" => Expression::NumberLiteral(1., Unit::None),
        "visible" => Expression::BoolLiteral(true),
        _ => return None,
    };
    Some(expr)
}

fn layout_constraint_prop(elem: &ElementRc, field: &str, orient: Orientation) -> Expression {
    let expr = match elem.borrow().layout_info_prop(orient) {
        Some(e) => Expression::PropertyReference(e.clone()),
        None => crate::layout::implicit_layout_info_call(elem, orient),
    };
    Expression::StructFieldAccess { base: expr.into(), name: field.into() }
}
