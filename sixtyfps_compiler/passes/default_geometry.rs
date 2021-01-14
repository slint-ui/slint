/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! Set the width and height of Rectangle, TouchArea, ... to 100%.

use std::rc::Rc;

use crate::expression_tree::{BuiltinFunction, Expression, NamedReference};
use crate::langtype::DefaultSizeBinding;
use crate::langtype::Type;
use crate::object_tree::{Component, ElementRc};

pub fn default_geometry(root_component: &Rc<Component>) {
    crate::object_tree::recurse_elem_including_sub_components(
        &root_component,
        &None,
        &mut |elem: &ElementRc, parent: &Option<ElementRc>| {
            let base_type = elem.borrow().base_type.clone();
            if let (Some(parent), Type::Builtin(builtin_type)) = (parent, base_type) {
                match builtin_type.default_size_binding {
                    DefaultSizeBinding::None => {}
                    DefaultSizeBinding::ExpandsToParentGeometry => {
                        if !elem.borrow().child_of_layout {
                            make_default_100(elem, parent, "width");
                            make_default_100(elem, parent, "height");
                        }
                    }
                    DefaultSizeBinding::ImplicitSize => {
                        make_default_implicit(elem, "width", BuiltinFunction::ImplicitItemSize);
                        make_default_implicit(elem, "height", BuiltinFunction::ImplicitItemSize);
                    }
                }
            }
            Some(elem.clone())
        },
    )
}

fn make_default_100(elem: &ElementRc, parent_element: &ElementRc, property: &str) {
    if parent_element.borrow().lookup_property(property) != Type::Length {
        return;
    }
    elem.borrow_mut().bindings.entry(property.into()).or_insert_with(|| {
        Expression::PropertyReference(NamedReference::new(parent_element, property)).into()
    });
}

fn make_default_implicit(elem: &ElementRc, property: &str, function: BuiltinFunction) {
    elem.borrow_mut().bindings.entry(property.into()).or_insert_with(|| {
        Expression::ObjectAccess {
            base: Expression::FunctionCall {
                function: Box::new(Expression::BuiltinFunctionReference(function)),
                arguments: vec![Expression::ElementReference(Rc::downgrade(elem))],
            }
            .into(),
            name: property.into(),
        }
        .into()
    });
}
