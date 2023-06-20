// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

//! This pass creates bindings to "absolute-y" and "absolute-y" properties
//! that can be used to compute the window-absolute coordinates of elements.

use std::cell::RefCell;
use std::rc::Rc;

use crate::expression_tree::{BuiltinFunction, Expression};
use crate::namedreference::NamedReference;
use crate::object_tree::{
    recurse_elem_including_sub_components_no_borrow, visit_all_named_references_in_element,
    Component, PropertyDeclaration,
};

pub fn lower_absolute_coordinates(component: &Rc<Component>) {
    let mut to_materialize = std::collections::HashSet::new();

    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        visit_all_named_references_in_element(elem, |nr| {
            if nr.name() == "absolute-position" {
                to_materialize.insert(nr.clone());
            }
        });
    });

    // Absolute item coordinates without the item local x/y.
    let cached_absolute_item_prop_name = "cached-absolute-xy".to_string();
    let point_type = match BuiltinFunction::ItemAbsolutePosition.ty() {
        crate::langtype::Type::Function { return_type, .. } => return_type.as_ref().clone(),
        _ => unreachable!(),
    };

    for nr in to_materialize {
        let elem = nr.element();

        elem.borrow_mut()
            .property_declarations
            .entry(cached_absolute_item_prop_name.clone())
            .or_insert_with(|| PropertyDeclaration {
                property_type: point_type.clone(),
                ..PropertyDeclaration::default()
            });

        if !elem.borrow().bindings.contains_key(&cached_absolute_item_prop_name) {
            let point_binding = Expression::FunctionCall {
                function: Box::new(Expression::BuiltinFunctionReference(
                    BuiltinFunction::ItemAbsolutePosition,
                    None,
                )),
                arguments: vec![Expression::ElementReference(Rc::downgrade(&elem))],
                source_location: None,
            };
            elem.borrow_mut()
                .bindings
                .insert(cached_absolute_item_prop_name.clone(), RefCell::new(point_binding.into()));
        }

        // Create a binding to the hidden point property and add item local x/y. The
        // materialize properties pass is going to create the actual property for
        // absolute-position.
        let binding = Expression::Struct {
            ty: point_type.clone(),
            values: IntoIterator::into_iter(["x", "y"])
                .map(|coord| {
                    (
                        coord.to_string(),
                        Expression::BinaryExpression {
                            lhs: Expression::StructFieldAccess {
                                base: Expression::PropertyReference(NamedReference::new(
                                    &elem,
                                    &cached_absolute_item_prop_name,
                                ))
                                .into(),
                                name: coord.to_string(),
                            }
                            .into(),
                            rhs: Expression::PropertyReference(NamedReference::new(&elem, &coord))
                                .into(),
                            op: '+',
                        },
                    )
                })
                .collect(),
        };
        elem.borrow_mut().bindings.insert(nr.name().to_string(), RefCell::new(binding.into()));
    }
}
