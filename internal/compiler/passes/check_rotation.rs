// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::BuildDiagnostics;
use crate::diagnostics::Spanned;
use crate::langtype::ElementType;
use crate::object_tree::Element;

/// Check that the rotation is only on Image and Text
pub fn check_rotation(doc: &crate::object_tree::Document, diag: &mut BuildDiagnostics) {
    for cmp in &doc.inner_components {
        crate::object_tree::recurse_elem_including_sub_components(cmp, &(), &mut |elem, _| {
            let e = elem.borrow();
            if crate::typeregister::RESERVED_ROTATION_PROPERTIES
                .iter()
                .any(|(property_name, _)| is_property_set(&e, property_name))
            {
                if !e.builtin_type().is_some_and(|b| matches!(b.name.as_str(), "Image" | "Text")) {
                    let span = e
                        .bindings
                        .get("rotation-angle")
                        .and_then(|e| e.borrow().span.clone())
                        .unwrap_or_else(|| e.to_source_location());

                    diag.push_error_with_span(
                        "rotation properties can only be applied to the Image or Text element"
                            .into(),
                        span,
                    );
                } else if has_any_children(&e) {
                    diag.push_error_with_span(
                        "Elements with rotation properties cannot have children elements".into(),
                        e.to_source_location(),
                    );
                }
            }
        });
    }
}

/// Returns true if this element or its base have any children.
fn has_any_children(e: &Element) -> bool {
    !e.children.is_empty()
        || matches!(&e.base_type, ElementType::Component(base) if has_any_children(&base.root_element.borrow()))
}

/// Returns true if the property is set.
fn is_property_set(e: &Element, property_name: &str) -> bool {
    e.bindings.contains_key(property_name)
        || e.property_analysis
            .borrow()
            .get(property_name)
            .map_or(false, |a| a.is_set || a.is_linked)
        || matches!(&e.base_type, ElementType::Component(base) if is_property_set(&base.root_element.borrow(), property_name))
}
