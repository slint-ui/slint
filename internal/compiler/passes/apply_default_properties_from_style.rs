// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass applies the default properties from the style.
//!
//! Note that the layout default properties are handled in the lower_layout pass

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Expression, NamedReference};
use crate::langtype::Type;
use crate::object_tree::Component;
use smol_str::SmolStr;
use std::rc::Rc;

/// Ideally we would be able to write this in builtin.slint, but the StyleMetrics is not available there
pub fn apply_default_properties_from_style(
    root_component: &Rc<Component>,
    style_metrics: &Rc<Component>,
    palette: &Rc<Component>,
    _diag: &mut BuildDiagnostics,
) {
    crate::object_tree::recurse_elem_including_sub_components(
        root_component,
        &(),
        &mut |elem, _| {
            let mut elem = elem.borrow_mut();
            match elem.builtin_type().as_ref().map_or("", |b| b.name.as_str()) {
                "TextInput" => {
                    elem.set_binding_if_not_set("text-cursor-width".into(), || {
                        Expression::PropertyReference(NamedReference::new(
                            &style_metrics.root_element,
                            SmolStr::new_static("text-cursor-width"),
                        ))
                    });
                    elem.set_binding_if_not_set("color".into(), || {
                        Expression::PropertyReference(NamedReference::new(
                            &palette.root_element,
                            SmolStr::new_static("foreground"),
                        ))
                        .into()
                    });
                    elem.set_binding_if_not_set("selection-background-color".into(), || {
                        Expression::Cast {
                            from: Expression::PropertyReference(NamedReference::new(
                                &palette.root_element,
                                SmolStr::new_static("selection-background"),
                            ))
                            .into(),
                            to: Type::Color,
                        }
                    });
                    elem.set_binding_if_not_set("selection-foreground-color".into(), || {
                        Expression::Cast {
                            from: Expression::PropertyReference(NamedReference::new(
                                &palette.root_element,
                                SmolStr::new_static("selection-foreground"),
                            ))
                            .into(),
                            to: Type::Color,
                        }
                    });
                }
                "Text" | "MarkdownText" => {
                    elem.set_binding_if_not_set("color".into(), || Expression::Cast {
                        from: Expression::PropertyReference(NamedReference::new(
                            &palette.root_element,
                            SmolStr::new_static("foreground"),
                        ))
                        .into(),
                        to: Type::Brush,
                    });
                }
                "Dialog" | "Window" => {
                    elem.set_binding_if_not_set("background".into(), || Expression::Cast {
                        from: Expression::PropertyReference(NamedReference::new(
                            &palette.root_element,
                            SmolStr::new_static("background"),
                        ))
                        .into(),
                        to: Type::Brush,
                    });

                    let mut bind_style_property_if_exists = |property_name, property_type| {
                        if !matches!(
                            style_metrics
                                .root_element
                                .borrow()
                                .lookup_property(property_name)
                                .property_type,
                            Type::Invalid,
                        ) {
                            elem.set_binding_if_not_set(property_name.into(), || {
                                Expression::Cast {
                                    from: Expression::PropertyReference(NamedReference::new(
                                        &style_metrics.root_element,
                                        SmolStr::new_static(property_name),
                                    ))
                                    .into(),
                                    to: property_type,
                                }
                            });
                        }
                    };

                    bind_style_property_if_exists("default-font-family", Type::String);
                }

                _ => {}
            }
        },
    )
}
