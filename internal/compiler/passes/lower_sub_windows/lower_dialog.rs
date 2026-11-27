// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Lowering of the `Dialog` element.

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::element_type::PropertyLookupMode;
use crate::expression_tree::{BindingExpression, Expression, NamedReference};
use crate::expression_types::Orientation;
use crate::langtype::Type;
use crate::layout::{GridLayout, Layout, LayoutGeometry, create_new_prop};
use crate::native_class::PropertyVisibility;
use crate::object_tree::*;
use crate::typeregister::layout_info_type;
use smol_str::{SmolStr, format_smolstr};
use std::collections::HashSet;
use std::rc::Rc;

/// Lay out the main widget of a `Dialog` and its buttons, in the platform's button order.
pub fn lower_dialog_layout(
    dialog_element: &ElementRc,
    style_metrics: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    let mut grid = GridLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(dialog_element),
        dialog_button_roles: None,
        uses_auto: true,
    };
    let metrics = &style_metrics.root_element;
    grid.geometry
        .padding
        .bottom
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-padding")));
    grid.geometry
        .padding
        .top
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-padding")));
    grid.geometry
        .padding
        .left
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-padding")));
    grid.geometry
        .padding
        .right
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-padding")));
    grid.geometry
        .spacing
        .horizontal
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-spacing")));
    grid.geometry
        .spacing
        .vertical
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-spacing")));

    let layout_organized_data_prop = create_new_prop(
        dialog_element,
        SmolStr::new_static("layout-organized-data"),
        Type::ArrayOfU16,
    );
    let layout_cache_prop_h =
        create_new_prop(dialog_element, SmolStr::new_static("layout-cache-h"), Type::ArrayOfCoord);
    let layout_cache_prop_v =
        create_new_prop(dialog_element, SmolStr::new_static("layout-cache-v"), Type::ArrayOfCoord);
    let layout_info_prop_h = create_new_prop(
        dialog_element,
        SmolStr::new_static("layoutinfo-h"),
        layout_info_type().into(),
    );
    let layout_info_prop_v = create_new_prop(
        dialog_element,
        SmolStr::new_static("layoutinfo-v"),
        layout_info_type().into(),
    );

    let mut main_widget = None;
    let mut button_roles = Vec::new();
    let mut seen_buttons = HashSet::new();
    let mut num_cached_items: usize = 0;
    let layout_children = std::mem::take(&mut dialog_element.borrow_mut().children);
    for layout_child in &layout_children {
        let dialog_button_role_binding =
            layout_child.borrow_mut().take_binding("dialog-button-role");
        let is_button = if let Some(role_binding) = dialog_button_role_binding {
            if let Expression::EnumerationValue(val) = role_binding.expression.ignore_debug_hooks()
            {
                let en = &val.enumeration;
                debug_assert_eq!(en.name, "DialogButtonRole");
                button_roles.push(en.values[val.value].clone());
                if val.value == 0 {
                    diag.push_error(
                        "The `dialog-button-role` cannot be set explicitly to none".into(),
                        &role_binding,
                    );
                }
            } else {
                diag.push_error(
                    "The `dialog-button-role` property must be known at compile-time".into(),
                    &role_binding,
                );
            }
            if crate::passes::lower_layout::is_standard_button(&layout_child.borrow()) {
                diag.push_warning(
                    "'dialog-button-role' overrides the role that a 'StandardButton' gets from its 'kind': remove it, or use a 'Button' for a custom role".into(),
                    &role_binding,
                );
            }
            true
        } else if crate::passes::lower_layout::is_standard_button(&layout_child.borrow()) {
            // layout_child is a StandardButton
            match layout_child.borrow().binding("kind") {
                None => diag.push_error(
                    "The `kind` property of the StandardButton in a Dialog must be set".into(),
                    &*layout_child.borrow(),
                ),
                Some(binding) => {
                    if let Expression::EnumerationValue(val) =
                        binding.expression.ignore_debug_hooks()
                    {
                        let en = &val.enumeration;
                        debug_assert_eq!(en.name, "StandardButtonKind");
                        let kind = &en.values[val.value];
                        let role = match kind.as_str() {
                            "ok" => "accept",
                            "cancel" => "reject",
                            "apply" => "apply",
                            "close" => "reject",
                            "reset" => "reset",
                            "help" => "help",
                            "yes" => "accept",
                            "no" => "reject",
                            "abort" => "reject",
                            "retry" => "accept",
                            "ignore" => "accept",
                            _ => unreachable!(),
                        };
                        button_roles.push(role.into());
                        if !seen_buttons.insert(val.value) {
                            diag.push_error("Duplicated `kind`: There are two StandardButton in this Dialog with the same kind".into(), &*binding);
                        } else if Rc::ptr_eq(
                            dialog_element,
                            &dialog_element
                                .borrow()
                                .enclosing_component
                                .upgrade()
                                .unwrap()
                                .root_element,
                        ) {
                            let clicked_ty = layout_child
                                .borrow()
                                .lookup_property("clicked", PropertyLookupMode::ComponentLocal)
                                .property_type;
                            if matches!(&clicked_ty, Type::Callback { .. })
                                && layout_child.borrow().binding("clicked").is_none_or(|c| {
                                    matches!(c.value_expression(), Expression::Invalid)
                                })
                            {
                                dialog_element
                                    .borrow_mut()
                                    .property_declarations
                                    .entry(format_smolstr!("{}-clicked", kind))
                                    .or_insert_with(|| PropertyDeclaration {
                                        property_type: clicked_ty,
                                        node: None,
                                        expose_in_public_api: true,
                                        is_alias: Some(NamedReference::new(
                                            layout_child,
                                            SmolStr::new_static("clicked"),
                                        )),
                                        visibility: PropertyVisibility::InOut,
                                        pure: None,
                                        shadowed_name: None,
                                        shadowable: false,
                                        testable: false,
                                        moved_from: None,
                                        deprecated: None,
                                        synthesized: false,
                                    });
                            }
                        }
                    } else {
                        diag.push_error(
                            "The `kind` property of the StandardButton in a Dialog must be known at compile-time"
                                .into(),
                            &*binding,
                        );
                    }
                }
            }
            true
        } else {
            false
        };

        if is_button {
            grid.add_element_with_coord(
                layout_child,
                (1, button_roles.len() as u16),
                (1, 1),
                &layout_cache_prop_h,
                &layout_cache_prop_v,
                &layout_organized_data_prop,
                diag,
                &mut num_cached_items,
            );
        } else if main_widget.is_some() {
            diag.push_error(
                "A Dialog can have only one child element that is not a StandardButton".into(),
                &*layout_child.borrow(),
            );
        } else {
            main_widget = Some(layout_child.clone())
        }
    }
    dialog_element.borrow_mut().children = layout_children;

    if let Some(main_widget) = main_widget {
        grid.add_element_with_coord(
            &main_widget,
            (0, 0),
            (1, button_roles.len() as u16 + 1),
            &layout_cache_prop_h,
            &layout_cache_prop_v,
            &layout_organized_data_prop,
            diag,
            &mut num_cached_items,
        );
    } else {
        diag.push_error(
            "A Dialog must have a single child element that is not StandardButton".into(),
            &*dialog_element.borrow(),
        );
    }
    grid.dialog_button_roles = Some(button_roles);

    let span = dialog_element.borrow().to_source_location();
    layout_organized_data_prop.element().borrow_mut().set_binding(
        layout_organized_data_prop.name().clone(),
        BindingExpression::new_with_span(
            Expression::OrganizeGridLayout(grid.clone()),
            span.clone(),
        ),
    );
    layout_cache_prop_h.element().borrow_mut().set_binding(
        layout_cache_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveGridLayout {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Horizontal,
            },
            span.clone(),
        ),
    );
    layout_cache_prop_v.element().borrow_mut().set_binding(
        layout_cache_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveGridLayout {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Vertical,
            },
            span.clone(),
        ),
    );
    layout_info_prop_h.element().borrow_mut().set_binding(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeGridLayoutInfo {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Horizontal,
                cross_axis_size: None,
            },
            span.clone(),
        ),
    );
    layout_info_prop_v.element().borrow_mut().set_binding(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeGridLayoutInfo {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Vertical,
                cross_axis_size: None,
            },
            span,
        ),
    );
    dialog_element.borrow_mut().layout_info_prop = Some((layout_info_prop_h, layout_info_prop_v));
    for d in dialog_element.borrow_mut().debug.iter_mut() {
        d.layout = Some(Layout::GridLayout(grid.clone()));
    }
}
