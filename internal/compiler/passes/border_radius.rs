// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that applies the default border-radius to border-top|bottom-left|right-radius.

use crate::diagnostics::BuildDiagnostics;
use crate::element_type::ElementType;
use crate::expression_tree::{Expression, NamedReference};
use crate::object_tree::{Document, ElementRc, recurse_elem_including_sub_components_no_borrow};
use by_address::ByAddress;
use smol_str::SmolStr;
use std::collections::HashSet;

pub const BORDER_RADIUS_PROPERTIES: [&str; 4] = [
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
];

pub fn handle_border_radius(doc: &Document, _diag: &mut BuildDiagnostics) {
    #[allow(clippy::mutable_key_type, reason = "ByAddress<ElementRc> keys hash by pointer")]
    let mut with_corner_radius = HashSet::new();
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if is_rectangle(elem) && uses_corner_radius(elem) {
                with_corner_radius.insert(ByAddress(native_element(elem)));
            }
        })
    });

    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if is_rectangle(elem)
                && elem.borrow().is_binding_set("border-radius", true)
                && with_corner_radius.contains(&ByAddress(native_element(elem)))
            {
                let border_radius = NamedReference::new(elem, SmolStr::new_static("border-radius"));
                for property_name in BORDER_RADIUS_PROPERTIES.iter() {
                    elem.borrow_mut().set_binding_if_not_set(SmolStr::new(property_name), || {
                        Expression::PropertyReference(border_radius.clone())
                    });
                }
            }
        })
    });
}

fn is_rectangle(elem: &ElementRc) -> bool {
    elem.borrow().builtin_type().is_some_and(|bty| bty.name == "Rectangle")
}

fn uses_corner_radius(elem: &ElementRc) -> bool {
    let elem = elem.borrow();
    BORDER_RADIUS_PROPERTIES.iter().any(|property_name| {
        elem.is_binding_set(property_name, true)
            || elem.named_references.is_referenced(property_name)
            || elem
                .binding_cell_including_synthetic(property_name)
                .is_some_and(|binding| binding.borrow().expression.is_synthetic_debug_hook())
    })
}

/// The element that becomes the native item, shared by all instances of a component:
/// the root of the innermost base component
fn native_element(elem: &ElementRc) -> ElementRc {
    match &elem.borrow().base_type {
        ElementType::Component(base) => native_element(&base.root_element),
        _ => elem.clone(),
    }
}
