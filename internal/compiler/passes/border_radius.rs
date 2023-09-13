// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! Pass that applies the default border-radius to border-top|bottom-left|right-radius.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Expression, NamedReference};
use crate::object_tree::Component;
use std::rc::Rc;

pub fn handle_border_radius(root_component: &Rc<Component>, _diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        root_component,
        &(),
        &mut |elem, _| {
            let bty = if let Some(bty) = elem.borrow().builtin_type() { bty } else { return };
            if bty.name == "Rectangle" && elem.borrow().is_binding_set("border-radius", true) {
                let border_radius = NamedReference::new(elem, "border-radius");
                for corner in ["top-left", "top-right", "bottom-right", "bottom-left"].iter() {
                    elem.borrow_mut()
                        .set_binding_if_not_set(format!("border-{}-radius", corner), || {
                            Expression::PropertyReference(border_radius.clone())
                        });
                }
            }
        },
    )
}
