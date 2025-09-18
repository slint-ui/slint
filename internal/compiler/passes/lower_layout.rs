// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass computes the layout constraint

use lyon_path::geom::euclid::approxeq::ApproxEq;

use crate::diagnostics::{BuildDiagnostics, DiagnosticLevel, Spanned};
use crate::expression_tree::*;
use crate::langtype::ElementType;
use crate::langtype::Type;
use crate::layout::*;
use crate::object_tree::*;
use crate::typeloader::TypeLoader;
use crate::typeregister::{layout_info_type, TypeRegister};
use smol_str::{format_smolstr, SmolStr};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

pub fn lower_layouts(
    component: &Rc<Component>,
    type_loader: &mut TypeLoader,
    style_metrics: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    // lower the preferred-{width, height}: 100%;
    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        if check_preferred_size_100(elem, "preferred-width", diag) {
            elem.borrow_mut().default_fill_parent.0 = true;
        }
        if check_preferred_size_100(elem, "preferred-height", diag) {
            elem.borrow_mut().default_fill_parent.1 = true;
        }
        let base = elem.borrow().sub_component().cloned();
        if let Some(base) = base {
            let base = base.root_element.borrow();
            let mut elem_mut = elem.borrow_mut();
            elem_mut.default_fill_parent.0 |= base.default_fill_parent.0;
            elem_mut.default_fill_parent.1 |= base.default_fill_parent.1;
        }
    });

    *component.root_constraints.borrow_mut() =
        LayoutConstraints::new(&component.root_element, diag, DiagnosticLevel::Error);

    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        let component = elem.borrow().enclosing_component.upgrade().unwrap();
        let is_layout = lower_element_layout(
            &component,
            elem,
            &type_loader.global_type_registry.borrow(),
            style_metrics,
            diag,
        );
        check_no_layout_properties(elem, is_layout, diag);
    });
}

fn check_preferred_size_100(elem: &ElementRc, prop: &str, diag: &mut BuildDiagnostics) -> bool {
    let ret = if let Some(p) = elem.borrow().bindings.get(prop) {
        if p.borrow().expression.ty() == Type::Percent {
            if !matches!(p.borrow().expression.ignore_debug_hooks(), Expression::NumberLiteral(val, _) if *val == 100.)
            {
                diag.push_error(
                    format!("{prop} must either be a length, or the literal '100%'"),
                    &*p.borrow(),
                );
            }
            true
        } else {
            false
        }
    } else {
        false
    };
    if ret {
        elem.borrow_mut().bindings.remove(prop).unwrap();
        return true;
    }
    false
}

/// If the element is a layout, lower it to a Rectangle, and set the geometry property of the element inside it.
/// Returns true if the element was a layout and has been lowered
fn lower_element_layout(
    component: &Rc<Component>,
    elem: &ElementRc,
    type_register: &TypeRegister,
    style_metrics: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) -> bool {
    let base_type = if let ElementType::Builtin(base_type) = &elem.borrow().base_type {
        base_type.clone()
    } else {
        return false;
    };
    match base_type.name.as_str() {
        "Row" => {
            // We shouldn't lower layout if we have a Row in there. Unless the Row is the root of a repeated item,
            // in which case another error has been reported
            assert!(
                diag.has_errors()
                    && Rc::ptr_eq(&component.root_element, elem)
                    && component
                        .parent_element
                        .upgrade()
                        .is_some_and(|e| e.borrow().repeated.is_some()),
                "Error should have been caught at element lookup time"
            );
            return false;
        }
        "GridLayout" => lower_grid_layout(component, elem, diag, type_register),
        "HorizontalLayout" => lower_box_layout(elem, diag, Orientation::Horizontal),
        "VerticalLayout" => lower_box_layout(elem, diag, Orientation::Vertical),
        "Dialog" => {
            lower_dialog_layout(elem, style_metrics, diag);
            return true; // the Dialog stays in the tree as a Dialog
        }
        _ => return false,
    };

    let mut elem = elem.borrow_mut();
    let elem = &mut *elem;
    let prev_base = std::mem::replace(&mut elem.base_type, type_register.empty_type());
    elem.default_fill_parent = (true, true);
    // Create fake properties for the layout properties
    for (p, ty) in prev_base.property_list() {
        if !elem.base_type.lookup_property(&p).is_valid()
            && !elem.property_declarations.contains_key(&p)
        {
            elem.property_declarations.insert(p, ty.into());
        }
    }

    true
}

fn lower_grid_layout(
    component: &Rc<Component>,
    grid_layout_element: &ElementRc,
    diag: &mut BuildDiagnostics,
    type_register: &TypeRegister,
) {
    let mut grid = GridLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(grid_layout_element),
        dialog_button_roles: None,
    };

    let layout_cache_prop_h = create_new_prop(
        grid_layout_element,
        SmolStr::new_static("layout-cache-h"),
        Type::LayoutCache,
    );
    let layout_cache_prop_v = create_new_prop(
        grid_layout_element,
        SmolStr::new_static("layout-cache-v"),
        Type::LayoutCache,
    );
    let layout_info_prop_h = create_new_prop(
        grid_layout_element,
        SmolStr::new_static("layoutinfo-h"),
        layout_info_type().into(),
    );
    let layout_info_prop_v = create_new_prop(
        grid_layout_element,
        SmolStr::new_static("layoutinfo-v"),
        layout_info_type().into(),
    );

    let mut row = 0;
    let mut col = 0;

    let layout_children = std::mem::take(&mut grid_layout_element.borrow_mut().children);
    let mut collected_children = Vec::new();
    for layout_child in layout_children {
        let is_row = if let ElementType::Builtin(be) = &layout_child.borrow().base_type {
            be.name == "Row"
        } else {
            false
        };
        if is_row {
            if col > 0 {
                row += 1;
                col = 0;
            }
            let row_children = std::mem::take(&mut layout_child.borrow_mut().children);
            for x in row_children {
                grid.add_element(
                    &x,
                    (&mut row, &mut col),
                    &layout_cache_prop_h,
                    &layout_cache_prop_v,
                    diag,
                );
                col += 1;
                collected_children.push(x);
            }
            if col > 0 {
                row += 1;
                col = 0;
            }
            if layout_child.borrow().has_popup_child {
                // We need to keep that element otherwise the popup will malfunction
                layout_child.borrow_mut().base_type = type_register.empty_type();
                collected_children.push(layout_child);
            } else {
                component.optimized_elements.borrow_mut().push(layout_child);
            }
        } else {
            grid.add_element(
                &layout_child,
                (&mut row, &mut col),
                &layout_cache_prop_h,
                &layout_cache_prop_v,
                diag,
            );
            col += 1;
            collected_children.push(layout_child);
        }
    }
    grid_layout_element.borrow_mut().children = collected_children;
    let span = grid_layout_element.borrow().to_source_location();
    layout_cache_prop_h.element().borrow_mut().bindings.insert(
        layout_cache_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveLayout(Layout::GridLayout(grid.clone()), Orientation::Horizontal),
            span.clone(),
        )
        .into(),
    );
    layout_cache_prop_v.element().borrow_mut().bindings.insert(
        layout_cache_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveLayout(Layout::GridLayout(grid.clone()), Orientation::Vertical),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(
                Layout::GridLayout(grid.clone()),
                Orientation::Horizontal,
            ),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(Layout::GridLayout(grid.clone()), Orientation::Vertical),
            span,
        )
        .into(),
    );
    grid_layout_element.borrow_mut().layout_info_prop =
        Some((layout_info_prop_h, layout_info_prop_v));
    for d in grid_layout_element.borrow_mut().debug.iter_mut() {
        d.layout = Some(Layout::GridLayout(grid.clone()));
    }
}

impl GridLayout {
    fn add_element(
        &mut self,
        item_element: &ElementRc,
        (row, col): (&mut u16, &mut u16),
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        diag: &mut BuildDiagnostics,
    ) {
        let mut get_const_value = |name: &str| {
            item_element
                .borrow_mut()
                .bindings
                .remove(name)
                .and_then(|e| eval_const_expr(&e.borrow().expression, name, &*e.borrow(), diag))
        };
        let colspan = get_const_value("colspan").unwrap_or(1);
        let rowspan = get_const_value("rowspan").unwrap_or(1);
        if let Some(r) = get_const_value("row") {
            *row = r;
            *col = 0;
        }
        if let Some(c) = get_const_value("col") {
            *col = c;
        }

        self.add_element_with_coord(
            item_element,
            (*row, *col),
            (rowspan, colspan),
            layout_cache_prop_h,
            layout_cache_prop_v,
            diag,
        )
    }

    fn add_element_with_coord(
        &mut self,
        item_element: &ElementRc,
        (row, col): (u16, u16),
        (rowspan, colspan): (u16, u16),
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        diag: &mut BuildDiagnostics,
    ) {
        let index = self.elems.len();
        if let Some(layout_item) = create_layout_item(item_element, diag) {
            if layout_item.repeater_index.is_some() {
                diag.push_error(
                    "'if' or 'for' expressions are not currently supported in grid layouts"
                        .to_string(),
                    &*item_element.borrow(),
                );
                return;
            }

            let e = &layout_item.elem;
            set_prop_from_cache(e, "x", layout_cache_prop_h, index * 2, &None, diag);
            if !layout_item.item.constraints.fixed_width {
                set_prop_from_cache(e, "width", layout_cache_prop_h, index * 2 + 1, &None, diag);
            }
            set_prop_from_cache(e, "y", layout_cache_prop_v, index * 2, &None, diag);
            if !layout_item.item.constraints.fixed_height {
                set_prop_from_cache(e, "height", layout_cache_prop_v, index * 2 + 1, &None, diag);
            }

            self.elems.push(GridLayoutElement {
                col,
                row,
                colspan,
                rowspan,
                item: layout_item.item,
            });
        }
    }
}

fn lower_box_layout(
    layout_element: &ElementRc,
    diag: &mut BuildDiagnostics,
    orientation: Orientation,
) {
    let mut layout = BoxLayout {
        orientation,
        elems: Default::default(),
        geometry: LayoutGeometry::new(layout_element),
    };

    let layout_cache_prop =
        create_new_prop(layout_element, SmolStr::new_static("layout-cache"), Type::LayoutCache);
    let layout_info_prop_v = create_new_prop(
        layout_element,
        SmolStr::new_static("layoutinfo-v"),
        layout_info_type().into(),
    );
    let layout_info_prop_h = create_new_prop(
        layout_element,
        SmolStr::new_static("layoutinfo-h"),
        layout_info_type().into(),
    );

    let layout_children = std::mem::take(&mut layout_element.borrow_mut().children);

    let (begin_padding, end_padding) = match orientation {
        Orientation::Horizontal => (&layout.geometry.padding.top, &layout.geometry.padding.bottom),
        Orientation::Vertical => (&layout.geometry.padding.left, &layout.geometry.padding.right),
    };
    let (pos, size, pad, ortho) = match orientation {
        Orientation::Horizontal => ("x", "width", "y", "height"),
        Orientation::Vertical => ("y", "height", "x", "width"),
    };
    let pad_expr = begin_padding.clone().map(Expression::PropertyReference);
    let mut size_expr = Expression::PropertyReference(NamedReference::new(
        layout_element,
        SmolStr::new_static(ortho),
    ));
    if let Some(p) = begin_padding {
        size_expr = Expression::BinaryExpression {
            lhs: Box::new(std::mem::take(&mut size_expr)),
            rhs: Box::new(Expression::PropertyReference(p.clone())),
            op: '-',
        }
    }
    if let Some(p) = end_padding {
        size_expr = Expression::BinaryExpression {
            lhs: Box::new(std::mem::take(&mut size_expr)),
            rhs: Box::new(Expression::PropertyReference(p.clone())),
            op: '-',
        }
    }

    for layout_child in &layout_children {
        if let Some(item) = create_layout_item(layout_child, diag) {
            let index = layout.elems.len() * 2;
            let rep_idx = &item.repeater_index;
            let (fixed_size, fixed_ortho) = match orientation {
                Orientation::Horizontal => {
                    (item.item.constraints.fixed_width, item.item.constraints.fixed_height)
                }
                Orientation::Vertical => {
                    (item.item.constraints.fixed_height, item.item.constraints.fixed_width)
                }
            };
            let actual_elem = &item.elem;
            set_prop_from_cache(actual_elem, pos, &layout_cache_prop, index, rep_idx, diag);
            if !fixed_size {
                set_prop_from_cache(
                    actual_elem,
                    size,
                    &layout_cache_prop,
                    index + 1,
                    rep_idx,
                    diag,
                );
            }
            if let Some(pad_expr) = pad_expr.clone() {
                actual_elem.borrow_mut().bindings.insert(pad.into(), RefCell::new(pad_expr.into()));
            }
            if !fixed_ortho {
                actual_elem
                    .borrow_mut()
                    .bindings
                    .insert(ortho.into(), RefCell::new(size_expr.clone().into()));
            }
            layout.elems.push(item.item);
        }
    }
    layout_element.borrow_mut().children = layout_children;
    let span = layout_element.borrow().to_source_location();
    layout_cache_prop.element().borrow_mut().bindings.insert(
        layout_cache_prop.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveLayout(Layout::BoxLayout(layout.clone()), orientation),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(
                Layout::BoxLayout(layout.clone()),
                Orientation::Horizontal,
            ),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(Layout::BoxLayout(layout.clone()), Orientation::Vertical),
            span,
        )
        .into(),
    );
    layout_element.borrow_mut().layout_info_prop = Some((layout_info_prop_h, layout_info_prop_v));
    for d in layout_element.borrow_mut().debug.iter_mut() {
        d.layout = Some(Layout::BoxLayout(layout.clone()));
    }
}

fn lower_dialog_layout(
    dialog_element: &ElementRc,
    style_metrics: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    let mut grid = GridLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(dialog_element),
        dialog_button_roles: None,
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

    let layout_cache_prop_h =
        create_new_prop(dialog_element, SmolStr::new_static("layout-cache-h"), Type::LayoutCache);
    let layout_cache_prop_v =
        create_new_prop(dialog_element, SmolStr::new_static("layout-cache-v"), Type::LayoutCache);
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
    let mut button_roles = vec![];
    let mut seen_buttons = HashSet::new();
    let layout_children = std::mem::take(&mut dialog_element.borrow_mut().children);
    for layout_child in &layout_children {
        let dialog_button_role_binding =
            layout_child.borrow_mut().bindings.remove("dialog-button-role");
        let is_button = if let Some(role_binding) = dialog_button_role_binding {
            let role_binding = role_binding.into_inner();
            if let Expression::EnumerationValue(val) =
                super::ignore_debug_hooks(&role_binding.expression)
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
            true
        } else if matches!(&layout_child.borrow().lookup_property("kind").property_type, Type::Enumeration(e) if e.name == "StandardButtonKind")
        {
            // layout_child is a StandardButton
            match layout_child.borrow().bindings.get("kind") {
                None => diag.push_error(
                    "The `kind` property of the StandardButton in a Dialog must be set".into(),
                    &*layout_child.borrow(),
                ),
                Some(binding) => {
                    let binding = &*binding.borrow();
                    if let Expression::EnumerationValue(val) =
                        super::ignore_debug_hooks(&binding.expression)
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
                            diag.push_error("Duplicated `kind`: There are two StandardButton in this Dialog with the same kind".into(), binding);
                        } else if Rc::ptr_eq(
                            dialog_element,
                            &dialog_element
                                .borrow()
                                .enclosing_component
                                .upgrade()
                                .unwrap()
                                .root_element,
                        ) {
                            let clicked_ty =
                                layout_child.borrow().lookup_property("clicked").property_type;
                            if matches!(&clicked_ty, Type::Callback { .. })
                                && layout_child.borrow().bindings.get("clicked").is_none_or(|c| {
                                    matches!(c.borrow().expression, Expression::Invalid)
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
                                    });
                            }
                        }
                    } else {
                        diag.push_error(
                            "The `kind` property of the StandardButton in a Dialog must be known at compile-time"
                                .into(),
                            binding,
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
                diag,
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
            diag,
        );
    } else {
        diag.push_error(
            "A Dialog must have a single child element that is not StandardButton".into(),
            &*dialog_element.borrow(),
        );
    }
    grid.dialog_button_roles = Some(button_roles);

    let span = dialog_element.borrow().to_source_location();
    layout_cache_prop_h.element().borrow_mut().bindings.insert(
        layout_cache_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveLayout(Layout::GridLayout(grid.clone()), Orientation::Horizontal),
            span.clone(),
        )
        .into(),
    );
    layout_cache_prop_v.element().borrow_mut().bindings.insert(
        layout_cache_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveLayout(Layout::GridLayout(grid.clone()), Orientation::Vertical),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(
                Layout::GridLayout(grid.clone()),
                Orientation::Horizontal,
            ),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(Layout::GridLayout(grid.clone()), Orientation::Vertical),
            span,
        )
        .into(),
    );
    dialog_element.borrow_mut().layout_info_prop = Some((layout_info_prop_h, layout_info_prop_v));
    for d in dialog_element.borrow_mut().debug.iter_mut() {
        d.layout = Some(Layout::GridLayout(grid.clone()));
    }
}

struct CreateLayoutItemResult {
    item: LayoutItem,
    elem: ElementRc,
    repeater_index: Option<Expression>,
}

/// Create a LayoutItem for the given `item_element`  returns None is the layout is empty
fn create_layout_item(
    item_element: &ElementRc,
    diag: &mut BuildDiagnostics,
) -> Option<CreateLayoutItemResult> {
    let fix_explicit_percent = |prop: &str, item: &ElementRc| {
        if !item.borrow().bindings.get(prop).is_some_and(|b| b.borrow().ty() == Type::Percent) {
            return;
        }
        let min_name = format_smolstr!("min-{}", prop);
        let max_name = format_smolstr!("max-{}", prop);
        let mut min_ref = BindingExpression::from(Expression::PropertyReference(
            NamedReference::new(item, min_name.clone()),
        ));
        let mut item = item.borrow_mut();
        let b = item.bindings.remove(prop).unwrap().into_inner();
        min_ref.span = b.span.clone();
        min_ref.priority = b.priority;
        item.bindings.insert(max_name.clone(), min_ref.into());
        item.bindings.insert(min_name.clone(), b.into());
        item.property_declarations.insert(
            min_name,
            PropertyDeclaration { property_type: Type::Percent, ..PropertyDeclaration::default() },
        );
        item.property_declarations.insert(
            max_name,
            PropertyDeclaration { property_type: Type::Percent, ..PropertyDeclaration::default() },
        );
    };
    fix_explicit_percent("width", item_element);
    fix_explicit_percent("height", item_element);

    item_element.borrow_mut().child_of_layout = true;
    let (repeater_index, actual_elem) = if let Some(r) = &item_element.borrow().repeated {
        let rep_comp = item_element.borrow().base_type.as_component().clone();
        fix_explicit_percent("width", &rep_comp.root_element);
        fix_explicit_percent("height", &rep_comp.root_element);

        *rep_comp.root_constraints.borrow_mut() =
            LayoutConstraints::new(&rep_comp.root_element, diag, DiagnosticLevel::Error);
        rep_comp.root_element.borrow_mut().child_of_layout = true;
        (
            Some(if r.is_conditional_element {
                Expression::NumberLiteral(0., Unit::None)
            } else {
                Expression::RepeaterIndexReference { element: Rc::downgrade(item_element) }
            }),
            rep_comp.root_element.clone(),
        )
    } else {
        (None, item_element.clone())
    };

    let constraints = LayoutConstraints::new(&actual_elem, diag, DiagnosticLevel::Error);
    Some(CreateLayoutItemResult {
        item: LayoutItem { element: item_element.clone(), constraints },
        elem: actual_elem,
        repeater_index,
    })
}

fn set_prop_from_cache(
    elem: &ElementRc,
    prop: &str,
    layout_cache_prop: &NamedReference,
    index: usize,
    repeater_index: &Option<Expression>,
    diag: &mut BuildDiagnostics,
) {
    let old = elem.borrow_mut().bindings.insert(
        prop.into(),
        BindingExpression::new_with_span(
            Expression::LayoutCacheAccess {
                layout_cache_prop: layout_cache_prop.clone(),
                index,
                repeater_index: repeater_index.as_ref().map(|x| Box::new(x.clone())),
            },
            layout_cache_prop.element().borrow().to_source_location(),
        )
        .into(),
    );
    if let Some(old) = old.map(RefCell::into_inner) {
        diag.push_error(
            format!("The property '{prop}' cannot be set for elements placed in this layout, because the layout is already setting it"),
            &old,
        );
    }
}

fn eval_const_expr(
    expression: &Expression,
    name: &str,
    span: &dyn crate::diagnostics::Spanned,
    diag: &mut BuildDiagnostics,
) -> Option<u16> {
    match super::ignore_debug_hooks(expression) {
        Expression::NumberLiteral(v, Unit::None) => {
            if *v < 0. || *v > u16::MAX as f64 || !v.trunc().approx_eq(v) {
                diag.push_error(format!("'{name}' must be a positive integer"), span);
                None
            } else {
                Some(*v as u16)
            }
        }
        Expression::Cast { from, .. } => eval_const_expr(from, name, span, diag),
        _ => {
            diag.push_error(format!("'{name}' must be an integer literal"), span);
            None
        }
    }
}

/// Checks that there is grid-layout specific properties left
fn check_no_layout_properties(item: &ElementRc, is_layout: bool, diag: &mut BuildDiagnostics) {
    for (prop, expr) in item.borrow().bindings.iter() {
        if matches!(prop.as_ref(), "col" | "row" | "colspan" | "rowspan") {
            diag.push_error(format!("{prop} used outside of a GridLayout"), &*expr.borrow());
        }
        if matches!(prop.as_ref(), "dialog-button-role") {
            diag.push_error(format!("{prop} used outside of a Dialog"), &*expr.borrow());
        }
        if !is_layout
            && matches!(
                prop.as_ref(),
                "padding" | "padding-left" | "padding-right" | "padding-top" | "padding-bottom"
            )
        {
            diag.push_warning(
                format!("{prop} only has effect on layout elements"),
                &*expr.borrow(),
            );
        }
    }
}

/// For fixed layout, we need to dissociate the width and the height property of the WindowItem from width and height property
/// in slint such that the width and height property are actually constants.
///
/// The Slint runtime will change the width and height property of the native WindowItem to match those of the actual
/// window, but we don't want that to happen if we have a fixed layout.
pub fn check_window_layout(component: &Rc<Component>) {
    if component.root_constraints.borrow().fixed_height {
        adjust_window_layout(component, "height");
    }
    if component.root_constraints.borrow().fixed_width {
        adjust_window_layout(component, "width");
    }
}

fn adjust_window_layout(component: &Rc<Component>, prop: &'static str) {
    let new_prop = crate::layout::create_new_prop(
        &component.root_element,
        format_smolstr!("fixed-{prop}"),
        Type::LogicalLength,
    );
    {
        let mut root = component.root_element.borrow_mut();
        if let Some(b) = root.bindings.remove(prop) {
            root.bindings.insert(new_prop.name().clone(), b);
        };
        let mut analysis = root.property_analysis.borrow_mut();
        if let Some(a) = analysis.remove(prop) {
            analysis.insert(new_prop.name().clone(), a);
        };
        drop(analysis);
        root.bindings.insert(
            prop.into(),
            RefCell::new(Expression::PropertyReference(new_prop.clone()).into()),
        );
    }

    let old_prop = NamedReference::new(&component.root_element, SmolStr::new_static(prop));
    crate::object_tree::visit_all_named_references(component, &mut |nr| {
        if nr == &old_prop {
            *nr = new_prop.clone()
        }
    });
}
