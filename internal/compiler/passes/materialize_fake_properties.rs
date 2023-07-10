// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This pass creates properties that are used but are otherwise not real.
//!
//! Must be run after lower_layout and default_geometry passes

use crate::diagnostics::Spanned;
use crate::expression_tree::{BindingExpression, Expression, Unit};
use crate::langtype::{ElementType, Type};
use crate::layout::Orientation;
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use std::collections::BTreeMap;
use std::rc::Rc;

pub fn materialize_fake_properties(component: &Rc<Component>) {
    let mut to_materialize = std::collections::HashMap::new();

    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        visit_all_named_references_in_element(elem, |nr| {
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
        for prop in elem.borrow().bindings.keys() {
            let nr = NamedReference::new(elem, prop);
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
            nr.name().to_owned(),
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
            match elem_mut.bindings.entry(nr.name().into()) {
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
    property_declarations: &BTreeMap<String, PropertyDeclaration>,
    base_type: &ElementType,
    prop: &str,
) -> Option<Type> {
    if property_declarations.contains_key(prop) {
        return None;
    }
    let has_declared_property = match base_type {
        ElementType::Component(c) => has_declared_property(&c.root_element.borrow(), prop),
        ElementType::Builtin(b) => b.properties.contains_key(prop),
        ElementType::Native(n) => {
            n.lookup_property(prop).map_or(false, |prop_type| prop_type.is_property_type())
        }
        ElementType::Global | ElementType::Error => false,
    };

    if !has_declared_property {
        let ty = crate::typeregister::reserved_property(prop).property_type;
        if ty != Type::Invalid {
            return Some(ty);
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
        ElementType::Builtin(b) => b.properties.contains_key(prop),
        ElementType::Native(n) => n.lookup_property(prop).is_some(),
        ElementType::Global | ElementType::Error => false,
    }
}

/// Initialize a sensible default binding for the now materialized property
pub fn initialize(elem: &ElementRc, name: &str) -> Option<Expression> {
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
