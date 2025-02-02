// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass creates bindings to "absolute-y" and "absolute-y" properties
//! that can be used to compute the window-absolute coordinates of elements.

use smol_str::SmolStr;
use std::cell::RefCell;
use std::rc::Rc;

use crate::expression_tree::{BuiltinFunction, Expression};
use crate::langtype::Type;
use crate::namedreference::NamedReference;
use crate::object_tree::{
    recurse_elem_including_sub_components_no_borrow, visit_all_named_references_in_element,
    Component,
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

    let Type::Struct(point_type) = BuiltinFunction::ItemAbsolutePosition.ty().return_type.clone()
    else {
        unreachable!()
    };

    for nr in to_materialize {
        let elem = nr.element();

        // Create a binding for the `absolute-position` property. The
        // materialize properties pass is going to create the actual property later.

        let parent_position_var = Box::new(Expression::ReadLocalVariable {
            name: "parent_position".into(),
            ty: point_type.clone().into(),
        });

        let binding = Expression::CodeBlock(vec![
            Expression::StoreLocalVariable {
                name: "parent_position".into(),
                value: Expression::FunctionCall {
                    function: BuiltinFunction::ItemAbsolutePosition.into(),
                    arguments: vec![Expression::ElementReference(Rc::downgrade(&elem))],
                    source_location: None,
                }
                .into(),
            },
            Expression::Struct {
                ty: point_type.clone(),
                values: IntoIterator::into_iter(["x", "y"])
                    .map(|coord| {
                        (
                            coord.into(),
                            Expression::BinaryExpression {
                                lhs: Expression::StructFieldAccess {
                                    base: parent_position_var.clone(),
                                    name: coord.into(),
                                }
                                .into(),
                                rhs: Expression::PropertyReference(NamedReference::new(
                                    &elem,
                                    SmolStr::new_static(coord),
                                ))
                                .into(),
                                op: '+',
                            },
                        )
                    })
                    .collect(),
            },
        ]);

        elem.borrow_mut().bindings.insert(nr.name().clone(), RefCell::new(binding.into()));
    }
}
