// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that applies the default border-radius/border-corner-shape to their
//! border-top|bottom-left|right-radius/corner-shape counterparts.

use crate::expression_tree::{Expression, NamedReference};
use crate::object_tree::{Component, ElementRc};
use smol_str::SmolStr;
use std::rc::Rc;

pub const BORDER_RADIUS_PROPERTIES: [&str; 4] = [
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
];

pub const CORNER_SHAPE_PROPERTIES: [&str; 4] = [
    "border-top-left-corner-shape",
    "border-top-right-corner-shape",
    "border-bottom-right-corner-shape",
    "border-bottom-left-corner-shape",
];

/// Fills in `per_corner_properties` with a binding to `uniform_property`, so it acts as their default.
///
/// Any per-corner radius or shape selects the native item that reads the per-corner properties
/// instead of `uniform_property`.
fn fan_out_to_corners(elem: &ElementRc, uniform_property: &str, per_corner_properties: [&str; 4]) {
    let bty = if let Some(bty) = elem.borrow().builtin_type() { bty } else { return };
    if bty.name == "Rectangle"
        && elem.borrow().is_binding_set(uniform_property, true)
        && BORDER_RADIUS_PROPERTIES.iter().chain(&CORNER_SHAPE_PROPERTIES).any(|property_name| {
            let elem = elem.borrow();
            elem.is_binding_set(property_name, true)
                || elem
                    .binding_cell_including_synthetic(property_name)
                    .is_some_and(|binding| binding.borrow().expression.is_synthetic_debug_hook())
        })
    {
        let uniform = NamedReference::new(elem, SmolStr::new(uniform_property));
        for property_name in per_corner_properties.iter() {
            elem.borrow_mut().set_binding_if_not_set(SmolStr::new(property_name), || {
                Expression::PropertyReference(uniform.clone())
            });
        }
    }
}

pub fn handle_border_radius(
    root_component: &Rc<Component>,
    _diag: &mut crate::diagnostics::BuildDiagnostics,
) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        root_component,
        &(),
        &mut |elem, _| {
            fan_out_to_corners(elem, "border-radius", BORDER_RADIUS_PROPERTIES);
            fan_out_to_corners(elem, "border-corner-shape", CORNER_SHAPE_PROPERTIES);
        },
    )
}
