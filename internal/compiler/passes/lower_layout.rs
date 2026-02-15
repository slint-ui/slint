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
use crate::typeregister::{TypeRegister, layout_info_type};
use smol_str::{SmolStr, format_smolstr};
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

    recurse_elem_including_sub_components(
        component,
        &Option::default(),
        &mut |elem, parent_layout_type| {
            let component = elem.borrow().enclosing_component.upgrade().unwrap();

            lower_element_layout(
                &component,
                elem,
                &type_loader.global_type_registry.borrow(),
                style_metrics,
                parent_layout_type,
                diag,
            )
        },
    );
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
/// Returns the name of the layout type if the element was a layout and has been lowered
fn lower_element_layout(
    component: &Rc<Component>,
    elem: &ElementRc,
    type_register: &TypeRegister,
    style_metrics: &Rc<Component>,
    parent_layout_type: &Option<SmolStr>,
    diag: &mut BuildDiagnostics,
) -> Option<SmolStr> {
    let layout_type = if let ElementType::Builtin(base_type) = &elem.borrow().base_type {
        Some(base_type.name.clone())
    } else {
        None
    };

    check_no_layout_properties(elem, &layout_type, parent_layout_type, diag);

    match layout_type.as_ref()?.as_str() {
        "Row" => return layout_type,
        "GridLayout" => lower_grid_layout(component, elem, diag, type_register),
        "HorizontalLayout" => lower_box_layout(elem, diag, Orientation::Horizontal),
        "VerticalLayout" => lower_box_layout(elem, diag, Orientation::Vertical),
        "FlexBoxLayout" => lower_flexbox_layout(elem, diag),
        "Dialog" => {
            lower_dialog_layout(elem, style_metrics, diag);
            // return now, the Dialog stays in the tree as a Dialog
            return layout_type;
        }
        _ => return None,
    };

    let mut elem = elem.borrow_mut();
    let elem = &mut *elem;
    let prev_base = std::mem::replace(&mut elem.base_type, type_register.empty_type());
    elem.default_fill_parent = (true, true);
    // Create fake properties for the layout properties
    // like alignment, spacing, spacing-horizontal, spacing-vertical
    for (p, ty) in prev_base.property_list() {
        if !elem.base_type.lookup_property(&p).is_valid()
            && !elem.property_declarations.contains_key(&p)
        {
            elem.property_declarations.insert(p, ty.into());
        }
    }

    layout_type
}

// to detect mixing auto and non-literal expressions in row/col values
#[derive(Debug, PartialEq, Eq)]
enum RowColExpressionType {
    Auto, // not specified
    Literal,
    RuntimeExpression,
}
impl RowColExpressionType {
    fn from_option_expr(
        expr: &Option<Expression>,
        is_number_literal: bool,
    ) -> RowColExpressionType {
        match expr {
            None => RowColExpressionType::Auto,
            Some(_) if is_number_literal => RowColExpressionType::Literal,
            Some(_) => RowColExpressionType::RuntimeExpression,
        }
    }
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
        uses_auto: false,
    };

    let layout_organized_data_prop = create_new_prop(
        grid_layout_element,
        SmolStr::new_static("layout-organized-data"),
        Type::ArrayOfU16,
    );
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

    let layout_children = std::mem::take(&mut grid_layout_element.borrow_mut().children);
    let mut collected_children = Vec::new();
    let mut new_row = false; // true until the first child of a Row, or the first item after an empty Row
    let mut numbering_type: Option<RowColExpressionType> = None;
    let mut num_cached_items: usize = 0;
    for layout_child in layout_children {
        let is_repeated_row = {
            if layout_child.borrow().repeated.is_some()
                && let ElementType::Component(comp) = &layout_child.borrow().base_type
            {
                match &comp.root_element.borrow().base_type {
                    ElementType::Builtin(b) => b.name == "Row",
                    _ => false,
                }
            } else {
                false
            }
        };
        if is_repeated_row {
            grid.add_repeated_row(
                &layout_child,
                &layout_cache_prop_h,
                &layout_cache_prop_v,
                &layout_organized_data_prop,
                diag,
                &mut num_cached_items,
            );
            collected_children.push(layout_child);
            new_row = true;
        } else if layout_child.borrow().base_type.type_name() == Some("Row") {
            new_row = true;
            let row_children = std::mem::take(&mut layout_child.borrow_mut().children);
            for row_child in row_children {
                if let Some(binding) = row_child.borrow_mut().bindings.get("row") {
                    diag.push_warning(
                        "The 'row' property cannot be used for elements inside a Row. This was accepted by previous versions of Slint, but may become an error in the future".to_string(),
                        &*binding.borrow(),
                    );
                }
                grid.add_element(
                    &row_child,
                    new_row,
                    &layout_cache_prop_h,
                    &layout_cache_prop_v,
                    &layout_organized_data_prop,
                    &mut numbering_type,
                    diag,
                    &mut num_cached_items,
                );
                collected_children.push(row_child);
                new_row = false;
            }
            new_row = true; // the end of a Row means the next item is the first of a new row
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
                new_row,
                &layout_cache_prop_h,
                &layout_cache_prop_v,
                &layout_organized_data_prop,
                &mut numbering_type,
                diag,
                &mut num_cached_items,
            );
            collected_children.push(layout_child);
            new_row = false;
        }
    }
    grid_layout_element.borrow_mut().children = collected_children;
    grid.uses_auto = numbering_type == Some(RowColExpressionType::Auto);
    let span = grid_layout_element.borrow().to_source_location();

    layout_organized_data_prop.element().borrow_mut().bindings.insert(
        layout_organized_data_prop.name().clone(),
        BindingExpression::new_with_span(
            Expression::OrganizeGridLayout(grid.clone()),
            span.clone(),
        )
        .into(),
    );
    layout_cache_prop_h.element().borrow_mut().bindings.insert(
        layout_cache_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveGridLayout {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Horizontal,
            },
            span.clone(),
        )
        .into(),
    );
    layout_cache_prop_v.element().borrow_mut().bindings.insert(
        layout_cache_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveGridLayout {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Vertical,
            },
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeGridLayoutInfo {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Horizontal,
            },
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeGridLayoutInfo {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Vertical,
            },
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
        new_row: bool,
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        organized_data_prop: &NamedReference,
        numbering_type: &mut Option<RowColExpressionType>,
        diag: &mut BuildDiagnostics,
        num_cached_items: &mut usize,
    ) {
        // Some compile-time checks
        {
            let mut check_expr = |name: &str| {
                let mut is_number_literal = false;
                let expr = item_element.borrow_mut().bindings.get(name).map(|e| {
                    let expr = &e.borrow().expression;
                    is_number_literal =
                        check_number_literal_is_positive_integer(expr, name, &*e.borrow(), diag);
                    expr.clone()
                });
                (expr, is_number_literal)
            };

            let (row_expr, row_is_number_literal) = check_expr("row");
            let (col_expr, col_is_number_literal) = check_expr("col");
            check_expr("rowspan");
            check_expr("colspan");

            let mut check_numbering_consistency =
                |expr_type: RowColExpressionType, prop_name: &str| {
                    if !matches!(expr_type, RowColExpressionType::Literal) {
                        if let Some(current_numbering_type) = numbering_type {
                            if *current_numbering_type != expr_type {
                                let element_ref = item_element.borrow();
                                let span: &dyn Spanned =
                                    if let Some(binding) = element_ref.bindings.get(prop_name) {
                                        &*binding.borrow()
                                    } else {
                                        &*element_ref
                                    };
                                diag.push_error(
                                    format!("Cannot mix auto-numbering and runtime expressions for the '{prop_name}' property"),
                                    span,
                                );
                            }
                        } else {
                            // Store the first auto or runtime expression case we see
                            *numbering_type = Some(expr_type);
                        }
                    }
                };

            let row_expr_type =
                RowColExpressionType::from_option_expr(&row_expr, row_is_number_literal);
            check_numbering_consistency(row_expr_type, "row");

            let col_expr_type =
                RowColExpressionType::from_option_expr(&col_expr, col_is_number_literal);
            check_numbering_consistency(col_expr_type, "col");
        }

        let propref = |name: &'static str| -> Option<RowColExpr> {
            let nr = crate::layout::binding_reference(item_element, name).map(|nr| {
                // similar to adjust_references in repeater_component.rs (which happened before these references existed)
                let e = nr.element();
                let mut nr = nr.clone();
                if e.borrow().repeated.is_some()
                    && let crate::langtype::ElementType::Component(c) = e.borrow().base_type.clone()
                {
                    nr = NamedReference::new(&c.root_element, nr.name().clone())
                };
                nr
            });
            nr.map(RowColExpr::Named)
        };

        let row_expr = propref("row");
        let col_expr = propref("col");
        let rowspan_expr = propref("rowspan");
        let colspan_expr = propref("colspan");

        self.add_element_with_coord_as_expr(
            item_element,
            new_row,
            (&row_expr, &col_expr),
            (&rowspan_expr, &colspan_expr),
            layout_cache_prop_h,
            layout_cache_prop_v,
            organized_data_prop,
            diag,
            num_cached_items,
        );
    }

    fn add_element_with_coord(
        &mut self,
        item_element: &ElementRc,
        (row, col): (u16, u16),
        (rowspan, colspan): (u16, u16),
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        organized_data_prop: &NamedReference,
        diag: &mut BuildDiagnostics,
        num_cached_items: &mut usize,
    ) {
        self.add_element_with_coord_as_expr(
            item_element,
            false, // new_row
            (&Some(RowColExpr::Literal(row)), &Some(RowColExpr::Literal(col))),
            (&Some(RowColExpr::Literal(rowspan)), &Some(RowColExpr::Literal(colspan))),
            layout_cache_prop_h,
            layout_cache_prop_v,
            organized_data_prop,
            diag,
            num_cached_items,
        )
    }

    fn add_repeated_row(
        &mut self,
        item_element: &ElementRc,
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        organized_data_prop: &NamedReference,
        diag: &mut BuildDiagnostics,
        num_cached_items: &mut usize,
    ) {
        let layout_item = create_layout_item(item_element, diag);
        if let ElementType::Component(comp) = &item_element.borrow().base_type {
            let repeated_children_count = comp.root_element.borrow().children.len();
            let mut children_layout_items = Vec::new();
            for child in &comp.root_element.borrow().children {
                if child.borrow().repeated.is_some() {
                    diag.push_error(
                        "'if' or 'for' expressions are not currently supported within repeated Row elements (https://github.com/slint-ui/slint/issues/10670)".into(),
                        &*child.borrow(),
                    );
                };

                let sub_item = create_layout_item(child, diag);

                // Read colspan and rowspan from the child element
                let propref = |name: &'static str, elem: &ElementRc| -> Option<RowColExpr> {
                    let nr = crate::layout::binding_reference(elem, name).map(|nr| {
                        let e = nr.element();
                        let mut nr = nr.clone();
                        if e.borrow().repeated.is_some()
                            && let crate::langtype::ElementType::Component(c) =
                                e.borrow().base_type.clone()
                        {
                            nr = NamedReference::new(&c.root_element, nr.name().clone())
                        };
                        nr
                    });
                    nr.map(RowColExpr::Named)
                };
                let colspan_expr = propref("colspan", child);
                let rowspan_expr = propref("rowspan", child);
                let child_grid_cell = Rc::new(RefCell::new(GridLayoutCell {
                    new_row: false,
                    col_expr: RowColExpr::Auto,
                    row_expr: RowColExpr::Auto,
                    colspan_expr: colspan_expr.unwrap_or(RowColExpr::Literal(1)),
                    rowspan_expr: rowspan_expr.unwrap_or(RowColExpr::Literal(1)),
                    child_items: None,
                }));
                child.borrow_mut().grid_layout_cell = Some(child_grid_cell);

                // The layout engine will set x,y,width,height,row,col for each of the repeated children
                set_properties_from_cache(
                    &sub_item.elem,
                    &sub_item.item.constraints,
                    layout_cache_prop_h,
                    layout_cache_prop_v,
                    organized_data_prop,
                    *num_cached_items,
                    &layout_item.repeater_index,
                    repeated_children_count,
                    (&None::<RowColExpr>, &None::<RowColExpr>),
                    diag,
                );
                children_layout_items.push(sub_item.item);

                *num_cached_items += 1;
            }
            // Add a single GridLayoutElement for the repeated Row
            let grid_layout_cell = Rc::new(RefCell::new(GridLayoutCell {
                new_row: true,
                col_expr: RowColExpr::Auto,
                row_expr: RowColExpr::Auto,
                colspan_expr: RowColExpr::Literal(1),
                rowspan_expr: RowColExpr::Literal(1),
                child_items: Some(children_layout_items),
            }));
            let grid_layout_element = GridLayoutElement {
                cell: grid_layout_cell.clone(),
                item: layout_item.item.clone(),
            };
            comp.root_element.borrow_mut().grid_layout_cell = Some(grid_layout_cell);
            self.elems.push(grid_layout_element);
        }
    }

    fn add_element_with_coord_as_expr(
        &mut self,
        item_element: &ElementRc,
        new_row: bool,
        (row_expr, col_expr): (&Option<RowColExpr>, &Option<RowColExpr>),
        (rowspan_expr, colspan_expr): (&Option<RowColExpr>, &Option<RowColExpr>),
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        organized_data_prop: &NamedReference,
        diag: &mut BuildDiagnostics,
        num_cached_items: &mut usize,
    ) {
        let layout_item = create_layout_item(item_element, diag);

        set_properties_from_cache(
            &layout_item.elem,
            &layout_item.item.constraints,
            layout_cache_prop_h,
            layout_cache_prop_v,
            organized_data_prop,
            *num_cached_items,
            &layout_item.repeater_index,
            1,
            (row_expr, col_expr),
            diag,
        );

        let expr_or_default = |expr: &Option<RowColExpr>, default: RowColExpr| -> RowColExpr {
            match expr {
                Some(RowColExpr::Literal(v)) => RowColExpr::Literal(*v),
                Some(RowColExpr::Named(nr)) => RowColExpr::Named(nr.clone()),
                Some(RowColExpr::Auto) => RowColExpr::Auto,
                None => default,
            }
        };

        let grid_layout_cell = Rc::new(RefCell::new(GridLayoutCell {
            new_row,
            col_expr: expr_or_default(col_expr, RowColExpr::Auto),
            row_expr: expr_or_default(row_expr, RowColExpr::Auto),
            colspan_expr: expr_or_default(colspan_expr, RowColExpr::Literal(1)),
            rowspan_expr: expr_or_default(rowspan_expr, RowColExpr::Literal(1)),
            child_items: None,
        }));
        let grid_layout_element =
            GridLayoutElement { cell: grid_layout_cell.clone(), item: layout_item.item.clone() };
        layout_item.elem.borrow_mut().grid_layout_cell = Some(grid_layout_cell);
        self.elems.push(grid_layout_element);
        *num_cached_items += 1;
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
        let item = create_layout_item(layout_child, diag);
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
        // step=1 for box layout items (single element per repeater iteration)
        set_prop_from_cache(actual_elem, pos, &layout_cache_prop, index, rep_idx, 2, diag);
        if !fixed_size {
            set_prop_from_cache(actual_elem, size, &layout_cache_prop, index + 1, rep_idx, 2, diag);
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
    layout_element.borrow_mut().children = layout_children;
    let span = layout_element.borrow().to_source_location();
    layout_cache_prop.element().borrow_mut().bindings.insert(
        layout_cache_prop.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveBoxLayout(layout.clone(), orientation),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeBoxLayoutInfo(layout.clone(), Orientation::Horizontal),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeBoxLayoutInfo(layout.clone(), Orientation::Vertical),
            span,
        )
        .into(),
    );
    layout_element.borrow_mut().layout_info_prop = Some((layout_info_prop_h, layout_info_prop_v));
    for d in layout_element.borrow_mut().debug.iter_mut() {
        d.layout = Some(Layout::BoxLayout(layout.clone()));
    }
}

fn lower_flexbox_layout(layout_element: &ElementRc, diag: &mut BuildDiagnostics) {
    // Warn if alignment is set to stretch, which behaves like start in flexbox
    // (CSS spec: justify-content:stretch acts as flex-start for flex items)
    if let Some(binding) = layout_element.borrow().bindings.get("alignment") {
        let binding = binding.borrow();
        if matches!(binding.expression.ignore_debug_hooks(),
            Expression::EnumerationValue(v) if v.enumeration.name == "LayoutAlignment"
                && v.enumeration.values[v.value] == "stretch")
        {
            diag.push_warning(
                "alignment: stretch has no effect on FlexBoxLayout".into(),
                &*binding,
            );
        }
    }

    let direction = crate::layout::binding_reference(layout_element, "flex-direction");
    let align_content = crate::layout::binding_reference(layout_element, "align-content");
    let align_items = crate::layout::binding_reference(layout_element, "align-items");

    let mut layout = crate::layout::FlexBoxLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(layout_element),
        direction,
        align_content,
        align_items,
    };

    // FlexBoxLayout needs 4 values per item: x, y, width, height
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

    for layout_child in &layout_children {
        let item = create_layout_item(layout_child, diag);
        let index = layout.elems.len() * 4; // 4 values per item: x, y, width, height
        let rep_idx = &item.repeater_index;
        let actual_elem = &item.elem;

        // Set x from cache[index]
        set_prop_from_cache(actual_elem, "x", &layout_cache_prop, index, rep_idx, 4, diag);
        // Set y from cache[index + 1]
        set_prop_from_cache(actual_elem, "y", &layout_cache_prop, index + 1, rep_idx, 4, diag);
        // Set width from cache[index + 2] if not fixed
        if !item.item.constraints.fixed_width {
            set_prop_from_cache(
                actual_elem,
                "width",
                &layout_cache_prop,
                index + 2,
                rep_idx,
                4,
                diag,
            );
        }
        // Set height from cache[index + 3] if not fixed
        if !item.item.constraints.fixed_height {
            set_prop_from_cache(
                actual_elem,
                "height",
                &layout_cache_prop,
                index + 3,
                rep_idx,
                4,
                diag,
            );
        }
        layout.elems.push(item.item);
    }
    layout_element.borrow_mut().children = layout_children;
    let span = layout_element.borrow().to_source_location();

    layout_cache_prop.element().borrow_mut().bindings.insert(
        layout_cache_prop.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveFlexBoxLayout(layout.clone()),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeFlexBoxLayoutInfo(layout.clone(), Orientation::Horizontal),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeFlexBoxLayoutInfo(layout.clone(), Orientation::Vertical),
            span,
        )
        .into(),
    );
    layout_element.borrow_mut().layout_info_prop = Some((layout_info_prop_h, layout_info_prop_v));
    for d in layout_element.borrow_mut().debug.iter_mut() {
        d.layout = Some(Layout::FlexBoxLayout(layout.clone()));
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
    let mut button_roles = Vec::new();
    let mut seen_buttons = HashSet::new();
    let mut num_cached_items: usize = 0;
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
    layout_organized_data_prop.element().borrow_mut().bindings.insert(
        layout_organized_data_prop.name().clone(),
        BindingExpression::new_with_span(
            Expression::OrganizeGridLayout(grid.clone()),
            span.clone(),
        )
        .into(),
    );
    layout_cache_prop_h.element().borrow_mut().bindings.insert(
        layout_cache_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveGridLayout {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Horizontal,
            },
            span.clone(),
        )
        .into(),
    );
    layout_cache_prop_v.element().borrow_mut().bindings.insert(
        layout_cache_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveGridLayout {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Vertical,
            },
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeGridLayoutInfo {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Horizontal,
            },
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeGridLayoutInfo {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Vertical,
            },
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
) -> CreateLayoutItemResult {
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
    CreateLayoutItemResult {
        item: LayoutItem { element: item_element.clone(), constraints },
        elem: actual_elem,
        repeater_index,
    }
}

fn set_prop_from_cache(
    elem: &ElementRc,
    prop: &str,
    layout_cache_prop: &NamedReference,
    index: usize,
    repeater_index: &Option<Expression>,
    entries_per_item: usize,
    diag: &mut BuildDiagnostics,
) {
    let old = elem.borrow_mut().bindings.insert(
        prop.into(),
        BindingExpression::new_with_span(
            Expression::LayoutCacheAccess {
                layout_cache_prop: layout_cache_prop.clone(),
                index,
                repeater_index: repeater_index.as_ref().map(|x| Box::new(x.clone())),
                entries_per_item,
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

/// Helper function to set grid layout properties (x, y, width, height, col, row)
fn set_properties_from_cache(
    elem: &ElementRc,
    constraints: &LayoutConstraints,
    layout_cache_prop_h: &NamedReference,
    layout_cache_prop_v: &NamedReference,
    organized_data_prop: &NamedReference,
    num_cached_items: usize,
    rep_idx: &Option<Expression>,
    repeated_children_count: usize,
    (row_expr, col_expr): (&Option<RowColExpr>, &Option<RowColExpr>),
    diag: &mut BuildDiagnostics,
) {
    let cache_idx = num_cached_items * 2;
    let nr = 2 * repeated_children_count; // number of entries per repeated item
    set_prop_from_cache(elem, "x", layout_cache_prop_h, cache_idx, rep_idx, nr, diag);
    if !constraints.fixed_width {
        set_prop_from_cache(elem, "width", layout_cache_prop_h, cache_idx + 1, rep_idx, nr, diag);
    }
    set_prop_from_cache(elem, "y", layout_cache_prop_v, cache_idx, rep_idx, nr, diag);
    if !constraints.fixed_height {
        set_prop_from_cache(elem, "height", layout_cache_prop_v, cache_idx + 1, rep_idx, nr, diag);
    }

    let org_index = num_cached_items * 4;
    let org_nr = 4 * repeated_children_count; // number of entries per repeated item
    if col_expr.is_none() {
        set_prop_from_cache(elem, "col", organized_data_prop, org_index, rep_idx, org_nr, diag);
    }
    if row_expr.is_none() {
        set_prop_from_cache(elem, "row", organized_data_prop, org_index + 2, rep_idx, org_nr, diag);
    }
}

// If it's a number literal, it must be a positive integer
// But also allow any other kind of expression
// Returns true for literals, false for other kinds of expressions
fn check_number_literal_is_positive_integer(
    expression: &Expression,
    name: &str,
    span: &dyn crate::diagnostics::Spanned,
    diag: &mut BuildDiagnostics,
) -> bool {
    match super::ignore_debug_hooks(expression) {
        Expression::NumberLiteral(v, Unit::None) => {
            if *v > u16::MAX as f64 || !v.trunc().approx_eq(v) {
                diag.push_error(format!("'{name}' must be a positive integer"), span);
            }
            true
        }
        Expression::UnaryOp { op: '-', sub } => {
            if let Expression::NumberLiteral(_, Unit::None) = super::ignore_debug_hooks(sub) {
                diag.push_error(format!("'{name}' must be a positive integer"), span);
            }
            true
        }
        Expression::Cast { from, .. } => {
            check_number_literal_is_positive_integer(from, name, span, diag)
        }
        _ => false,
    }
}

fn recognized_layout_types() -> &'static [&'static str] {
    &["Row", "GridLayout", "HorizontalLayout", "VerticalLayout", "FlexBoxLayout", "Dialog"]
}

/// Checks that there are no grid-layout specific properties used wrongly
fn check_no_layout_properties(
    item: &ElementRc,
    layout_type: &Option<SmolStr>,
    parent_layout_type: &Option<SmolStr>,
    diag: &mut BuildDiagnostics,
) {
    let elem = item.borrow();
    for (prop, expr) in elem.bindings.iter() {
        if !matches!(parent_layout_type.as_deref(), Some("GridLayout") | Some("Row"))
            && matches!(prop.as_ref(), "col" | "row" | "colspan" | "rowspan")
        {
            diag.push_error(format!("{prop} used outside of a GridLayout's cell"), &*expr.borrow());
        }
        if parent_layout_type.as_deref() != Some("Dialog")
            && matches!(prop.as_ref(), "dialog-button-role")
        {
            diag.push_error(
                format!("{prop} used outside of a Dialog's direct child"),
                &*expr.borrow(),
            );
        }
        if (layout_type.is_none()
            || !recognized_layout_types().contains(&layout_type.as_ref().unwrap().as_str()))
            && matches!(
                prop.as_ref(),
                "padding" | "padding-left" | "padding-right" | "padding-top" | "padding-bottom"
            )
            && !check_inherits_layout(item)
        {
            diag.push_warning(
                format!("{prop} only has effect on layout elements"),
                &*expr.borrow(),
            );
        }
    }

    /// Check if the element inherits from a layout that was lowered
    fn check_inherits_layout(item: &ElementRc) -> bool {
        if let ElementType::Component(c) = &item.borrow().base_type {
            c.root_element.borrow().debug.iter().any(|d| d.layout.is_some())
                || check_inherits_layout(&c.root_element)
        } else {
            false
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
