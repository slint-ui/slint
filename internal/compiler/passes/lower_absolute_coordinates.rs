// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

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
            if nr.name() == "absolute-x" || nr.name() == "absolute-y" {
                to_materialize.insert(nr.clone());
            }
        });
    });

    let absolute_point_prop_name = "cached-absolute-xy".to_string();
    let point_type = crate::typeregister::logical_point_type();

    for nr in to_materialize {
        let elem = nr.element();

        elem.borrow_mut()
            .property_declarations
            .entry(absolute_point_prop_name.clone())
            .or_insert_with(|| PropertyDeclaration {
                property_type: point_type.clone(),
                ..PropertyDeclaration::default()
            });

        if !elem.borrow().bindings.contains_key(&absolute_point_prop_name) {
            let point_binding = Expression::FunctionCall {
                function: Box::new(Expression::BuiltinFunctionReference(
                    BuiltinFunction::MapPointToWindow,
                    None,
                )),
                arguments: vec![
                    Expression::ElementReference(Rc::downgrade(&elem)),
                    Expression::Struct {
                        ty: point_type.clone(),
                        values: ["x", "y"]
                            .into_iter()
                            .map(|coord_name| {
                                let coord_ref = NamedReference::new(&elem, coord_name);
                                (coord_name.to_string(), Expression::PropertyReference(coord_ref))
                            })
                            .collect(),
                    },
                ],
                source_location: None,
            };
            elem.borrow_mut()
                .bindings
                .insert(absolute_point_prop_name.clone(), RefCell::new(point_binding.into()));
        }

        // Create a binding to the hidden point property. The materialize properties pass is going to create the actual property
        // for absolute-x/y.
        let binding = Expression::StructFieldAccess {
            base: Expression::PropertyReference(NamedReference::new(
                &elem,
                &absolute_point_prop_name,
            ))
            .into(),
            name: nr.name().strip_prefix("absolute-").unwrap().to_string(),
        };
        elem.borrow_mut().bindings.insert(nr.name().to_string(), RefCell::new(binding.into()));
    }
}
