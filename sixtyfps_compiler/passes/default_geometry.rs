/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! This pass sets the width and heights of item which don't have their size specified
//! (neither explicitly, not in a layout)
//! The size is set according to the size in the element's DefaultSizeBinding.
//! If there is a layout within the object, the size will be a binding that depends on the layout
//! constraints.
//!
//! The pass must be run before the materialize_fake_properties pass. Since that pass needs to be
//! run before the lower_layout pass, it means we can't rely on the information from lower_layout,
//! and we will detect layouts in this pass

use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, Expression, NamedReference};
use crate::langtype::Type;
use crate::langtype::{DefaultSizeBinding, PropertyLookupResult};
use crate::object_tree::{Component, ElementRc};

pub fn default_geometry(root_component: &Rc<Component>, _diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components(
        &root_component,
        &None,
        &mut |elem: &ElementRc, parent: &Option<ElementRc>| {
            if !is_layout(&elem.borrow().base_type)
                && !parent.as_ref().map_or(false, |p| is_layout(&p.borrow().base_type))
            {
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
                            make_default_implicit(
                                elem,
                                "height",
                                BuiltinFunction::ImplicitItemSize,
                            );
                        }
                    }
                }
            }
            Some(elem.clone())
        },
    )
}

/// Return true if this type is a layout that sets the geometry (width/height) of its children, and that has constraints
fn is_layout(base_type: &Type) -> bool {
    if let Type::Builtin(be) = base_type {
        match be.native_class.class_name.as_str() {
            "Row" | "GridLayout" | "HorizontalLayout" | "VerticalLayout" => true,
            "PathLayout" => true,
            _ => false,
        }
    } else {
        false
    }
}

fn make_default_100(elem: &ElementRc, parent_element: &ElementRc, property: &str) {
    let PropertyLookupResult { resolved_name, property_type } =
        parent_element.borrow().lookup_property(property);
    if property_type != Type::Length {
        return;
    }
    elem.borrow_mut().bindings.entry(resolved_name.to_string()).or_insert_with(|| {
        Expression::PropertyReference(NamedReference::new(parent_element, resolved_name.as_ref()))
            .into()
    });
}

fn make_default_implicit(elem: &ElementRc, property: &str, function: BuiltinFunction) {
    elem.borrow_mut().bindings.entry(property.into()).or_insert_with(|| {
        Expression::StructFieldAccess {
            base: Expression::FunctionCall {
                function: Box::new(Expression::BuiltinFunctionReference(function)),
                arguments: vec![Expression::ElementReference(Rc::downgrade(elem))],
                source_location: None,
            }
            .into(),
            name: property.into(),
        }
        .into()
    });
}
