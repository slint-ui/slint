// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass handles the deprecated `rotation-origin-*` properties on Text and Image that were replaced by `transform-origin`

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::Expression;
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, ElementRc};
use smol_str::SmolStr;

pub fn handle_rotation_origin(component: &Component, diag: &mut BuildDiagnostics) {
    let transform_origin = crate::typeregister::transform_origin_property();

    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |elem, _| {
            let mut must_materialize = false;
            let mut seen = false;
            for (prop, _) in crate::typeregister::DEPRECATED_ROTATION_ORIGIN_PROPERTIES {
                if elem.borrow().is_property_set(prop) {
                    let span = match elem
                        .borrow()
                        .bindings
                        .get(prop)
                        .and_then(|b| b.borrow().span.clone())
                    {
                        Some(span) => span,
                        None => {
                            if seen {
                                return;
                            }
                            elem.borrow().to_source_location()
                        }
                    };

                    seen = true;

                    if !is_image_or_text(elem) {
                        diag.push_error(format!("'{prop}' cannot be set on this element"), &span);
                    } else {
                        diag.push_property_deprecation_warning(prop, transform_origin.0, &span);
                        must_materialize = true;
                    }
                }
            }
            if !must_materialize {
                return;
            }

            let expr = Expression::Struct {
                ty: transform_origin.1.clone(),
                values: crate::typeregister::DEPRECATED_ROTATION_ORIGIN_PROPERTIES
                    .iter()
                    .map(|(prop, _)| {
                        (
                            SmolStr::new_static(&prop[prop.len() - 1..]),
                            Expression::PropertyReference(NamedReference::new(
                                elem,
                                SmolStr::new_static(prop),
                            )),
                        )
                    })
                    .collect(),
            };

            // set_binding_overwriting upgrades a synthetic debug hook placeholder in place
            // and only reports a real (user-written) binding as a conflict.
            if let Some(old_binding) =
                elem.borrow_mut().set_binding_overwriting(transform_origin.0.into(), expr.into())
            {
                diag.push_error(
                    "Can't specify transform-origin if rotation-origin-x or rotation-origin-y is used on this element".into(),
                    &old_binding,
                );
            }
        },
    );
}

/// true if this element had a rotation-origin property
fn is_image_or_text(e: &ElementRc) -> bool {
    e.borrow().builtin_type().is_some_and(|bt| matches!(bt.name.as_str(), "Image" | "Text"))
}
