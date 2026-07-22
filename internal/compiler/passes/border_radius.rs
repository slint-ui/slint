// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that applies the default border-radius to border-top|bottom-left|right-radius.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Expression, NamedReference};
use crate::object_tree::Component;
use smol_str::SmolStr;
use std::rc::Rc;

pub const BORDER_RADIUS_PROPERTIES: [&str; 4] = [
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
];

pub fn handle_border_radius(root_component: &Rc<Component>, _diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        root_component,
        &(),
        &mut |elem, _| {
            let bty = if let Some(bty) = elem.borrow().builtin_type() { bty } else { return };
            if bty.name == "Rectangle"
                && elem.borrow().is_binding_set("border-radius", true)
                && BORDER_RADIUS_PROPERTIES
                    .iter()
                    .any(|property_name| elem.borrow().is_binding_set(property_name, true))
            {
                let border_radius = NamedReference::new(elem, SmolStr::new_static("border-radius"));
                for property_name in BORDER_RADIUS_PROPERTIES.iter() {
                    elem.borrow_mut().set_binding_if_not_set(SmolStr::new(property_name), || {
                        Expression::PropertyReference(border_radius.clone())
                    });
                }
            }
        },
    )
}
