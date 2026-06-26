// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass warns about a `DragArea` that never starts a drag, either because
//! it permits no drag action (none of `allow-copy`, `allow-move`, or
//! `allow-link` is set) or because its `data` payload is left unset.

use crate::diagnostics::BuildDiagnostics;
use crate::object_tree::{Component, ElementRc};

const ALLOW_PROPERTIES: [&str; 3] = ["allow-copy", "allow-move", "allow-link"];

pub fn check_drag_area(component: &Component, diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |elem, _| {
            if !is_drag_area(elem) {
                return;
            }
            if !ALLOW_PROPERTIES.iter().any(|prop| elem.borrow().is_property_set(prop)) {
                diag.push_warning(
                    "This 'DragArea' permits no drag action and will never start a drag; \
                     set 'allow-copy', 'allow-move', or 'allow-link' to true"
                        .into(),
                    &*elem.borrow(),
                );
            }
            if !elem.borrow().is_property_set("data") {
                diag.push_warning(
                    "This 'DragArea' has no 'data' set and will never start a drag; \
                     add a binding for the 'data' property"
                        .into(),
                    &*elem.borrow(),
                );
            }
        },
    );
}

fn is_drag_area(e: &ElementRc) -> bool {
    e.borrow().builtin_type().is_some_and(|bt| bt.name.as_str() == "DragArea")
}
