/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! Replace expression such as  `"width: 30%;` with `width: 0.3 * parent.width;`

use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Expression, NamedReference};
use crate::langtype::PropertyLookupResult;
use crate::langtype::Type;
use crate::object_tree::{Component, ElementRc};

pub fn apply_percentage_size(root_component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components(
        &root_component,
        &None,
        &mut |elem: &ElementRc, parent: &Option<ElementRc>| {
            fix_percent_size(elem, parent, "width", diag);
            fix_percent_size(elem, parent, "height", diag);
            Some(elem.clone())
        },
    )
}

fn fix_percent_size(
    elem: &ElementRc,
    parent: &Option<ElementRc>,
    property: &str,
    diag: &mut BuildDiagnostics,
) {
    if !elem.borrow().bindings.get(property).map_or(false, |b| b.ty() == Type::Percent) {
        return;
    }
    let mut elem = elem.borrow_mut();
    let b = elem.bindings.get_mut(property).unwrap();
    if let Some(parent) = parent {
        debug_assert_eq!(
            parent.borrow().lookup_property(property),
            PropertyLookupResult { resolved_name: property.into(), property_type: Type::Length }
        );
        b.expression = Expression::BinaryExpression {
            lhs: Box::new(std::mem::take(&mut b.expression).maybe_convert_to(
                Type::Float32,
                &b.span,
                diag,
            )),
            rhs: Box::new(Expression::PropertyReference(NamedReference::new(parent, property))),
            op: '*',
        }
    } else {
        diag.push_error("Cannot find parent property to apply relative lenght".into(), &b.span);
    }
}
