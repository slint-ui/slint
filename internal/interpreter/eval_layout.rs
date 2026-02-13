// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Value;
use crate::dynamic_item_tree::InstanceRef;
use crate::eval::{self, EvalLocalContext};
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::Type;
use i_slint_compiler::layout::{
    BoxLayout, GridLayout, LayoutConstraints, LayoutGeometry, Orientation, RowColExpr,
};
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_core::Coord;
use i_slint_core::items::{DialogButtonRole, FlexDirection, ItemRc};
use i_slint_core::layout::{self as core_layout, GridLayoutOrganizedData};
use i_slint_core::model::RepeatedItemTree;
use i_slint_core::slice::Slice;
use i_slint_core::window::WindowAdapter;
use std::rc::Rc;
use std::str::FromStr;

pub(crate) fn to_runtime(o: Orientation) -> core_layout::Orientation {
    match o {
        Orientation::Horizontal => core_layout::Orientation::Horizontal,
        Orientation::Vertical => core_layout::Orientation::Vertical,
    }
}

pub(crate) fn from_runtime(o: core_layout::Orientation) -> Orientation {
    match o {
        core_layout::Orientation::Horizontal => Orientation::Horizontal,
        core_layout::Orientation::Vertical => Orientation::Vertical,
    }
}

pub(crate) fn compute_grid_layout_info(
    grid_layout: &GridLayout,
    organized_data: &GridLayoutOrganizedData,
    orientation: Orientation,
    local_context: &mut EvalLocalContext,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    let (padding, spacing) = padding_and_spacing(&grid_layout.geometry, orientation, &expr_eval);
    let repeater_indices = grid_repeater_indices(grid_layout, local_context);
    let repeater_steps = grid_repeater_steps(grid_layout, local_context);
    let constraints = grid_layout_constraints(grid_layout, orientation, local_context);
    core_layout::grid_layout_info(
        organized_data.clone(),
        Slice::from_slice(constraints.as_slice()),
        Slice::from_slice(repeater_indices.as_slice()),
        Slice::from_slice(repeater_steps.as_slice()),
        spacing,
        &padding,
        to_runtime(orientation),
    )
    .into()
}

pub(crate) fn compute_box_layout_info(
    box_layout: &BoxLayout,
    orientation: Orientation,
    local_context: &mut EvalLocalContext,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    let (cells, alignment) = box_layout_data(box_layout, orientation, component, &expr_eval, None);
    let (padding, spacing) = padding_and_spacing(&box_layout.geometry, orientation, &expr_eval);
    if orientation == box_layout.orientation {
        core_layout::box_layout_info(Slice::from(cells.as_slice()), spacing, &padding, alignment)
    } else {
        core_layout::box_layout_info_ortho(Slice::from(cells.as_slice()), &padding)
    }
    .into()
}

pub(crate) fn organize_grid_layout(
    layout: &GridLayout,
    local_context: &mut EvalLocalContext,
) -> Value {
    let cells = grid_layout_input_data(layout, local_context);
    let repeater_indices = grid_repeater_indices(layout, local_context);
    let repeater_steps = grid_repeater_steps(layout, local_context);
    if let Some(buttons_roles) = &layout.dialog_button_roles {
        let roles = buttons_roles
            .iter()
            .map(|r| DialogButtonRole::from_str(r).unwrap())
            .collect::<Vec<_>>();
        core_layout::organize_dialog_button_layout(
            Slice::from_slice(cells.as_slice()),
            Slice::from_slice(roles.as_slice()),
        )
        .into()
    } else {
        core_layout::organize_grid_layout(
            Slice::from_slice(cells.as_slice()),
            Slice::from_slice(repeater_indices.as_slice()),
            Slice::from_slice(repeater_steps.as_slice()),
        )
        .into()
    }
}

pub(crate) fn solve_grid_layout(
    organized_data: &GridLayoutOrganizedData,
    grid_layout: &GridLayout,
    orientation: Orientation,
    local_context: &mut EvalLocalContext,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    let repeater_indices = grid_repeater_indices(grid_layout, local_context);
    let repeater_steps = grid_repeater_steps(grid_layout, local_context);
    let constraints = grid_layout_constraints(grid_layout, orientation, local_context);

    let (padding, spacing) = padding_and_spacing(&grid_layout.geometry, orientation, &expr_eval);
    let size_ref = grid_layout.geometry.rect.size_reference(orientation);

    let data = core_layout::GridLayoutData {
        size: size_ref.map(expr_eval).unwrap_or(0.),
        spacing,
        padding,
        organized_data: organized_data.clone(),
    };

    core_layout::solve_grid_layout(
        &data,
        Slice::from_slice(constraints.as_slice()),
        to_runtime(orientation),
        Slice::from_slice(repeater_indices.as_slice()),
        Slice::from_slice(repeater_steps.as_slice()),
    )
    .into()
}

pub(crate) fn solve_box_layout(
    box_layout: &BoxLayout,
    orientation: Orientation,
    local_context: &mut EvalLocalContext,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };

    let mut repeated_indices = Vec::new();
    let (cells, alignment) = box_layout_data(
        box_layout,
        orientation,
        component,
        &expr_eval,
        Some(&mut repeated_indices),
    );
    let (padding, spacing) = padding_and_spacing(&box_layout.geometry, orientation, &expr_eval);
    let size_ref = match orientation {
        Orientation::Horizontal => &box_layout.geometry.rect.width_reference,
        Orientation::Vertical => &box_layout.geometry.rect.height_reference,
    };
    core_layout::solve_box_layout(
        &core_layout::BoxLayoutData {
            size: size_ref.as_ref().map(expr_eval).unwrap_or(0.),
            spacing,
            padding,
            alignment,
            cells: Slice::from(cells.as_slice()),
        },
        Slice::from(repeated_indices.as_slice()),
    )
    .into()
}

pub(crate) fn solve_flexbox_layout(
    flexbox_layout: &i_slint_compiler::layout::FlexBoxLayout,
    local_context: &mut EvalLocalContext,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };

    let (cells_h, cells_v, repeated_indices) =
        flexbox_layout_data(flexbox_layout, component, &expr_eval, local_context);

    let width_ref = &flexbox_layout.geometry.rect.width_reference;
    let height_ref = &flexbox_layout.geometry.rect.height_reference;
    let alignment = flexbox_layout
        .geometry
        .alignment
        .as_ref()
        .map_or(i_slint_core::items::LayoutAlignment::default(), |nr| {
            eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
        });
    let direction = flexbox_layout_direction(flexbox_layout, local_context);

    let (padding_h, spacing_h) =
        padding_and_spacing(&flexbox_layout.geometry, Orientation::Horizontal, &expr_eval);
    let (padding_v, spacing_v) =
        padding_and_spacing(&flexbox_layout.geometry, Orientation::Vertical, &expr_eval);

    core_layout::solve_flexbox_layout(
        &core_layout::FlexBoxLayoutData {
            width: width_ref.as_ref().map(&expr_eval).unwrap_or(0.),
            height: height_ref.as_ref().map(&expr_eval).unwrap_or(0.),
            spacing_h,
            spacing_v,
            padding_h,
            padding_v,
            alignment,
            direction,
            cells_h: Slice::from(cells_h.as_slice()),
            cells_v: Slice::from(cells_v.as_slice()),
        },
        Slice::from(repeated_indices.as_slice()),
    )
    .into()
}

fn flexbox_layout_direction(
    flexbox_layout: &i_slint_compiler::layout::FlexBoxLayout,
    local_context: &EvalLocalContext,
) -> FlexDirection {
    flexbox_layout
        .direction
        .as_ref()
        .and_then(|nr| {
            let value =
                eval::load_property(local_context.component_instance, &nr.element(), nr.name())
                    .ok()?;
            if let Value::EnumerationValue(_, variant) = &value {
                match variant.as_str() {
                    "row" => Some(FlexDirection::Row),
                    "row-reverse" => Some(FlexDirection::RowReverse),
                    "column" => Some(FlexDirection::Column),
                    "column-reverse" => Some(FlexDirection::ColumnReverse),
                    _ => None,
                }
            } else {
                None
            }
        })
        .unwrap_or(FlexDirection::Row)
}

pub(crate) fn compute_flexbox_layout_info(
    flexbox_layout: &i_slint_compiler::layout::FlexBoxLayout,
    orientation: Orientation,
    local_context: &mut EvalLocalContext,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };

    let (cells_h, cells_v, _repeated_indices) =
        flexbox_layout_data(flexbox_layout, component, &expr_eval, local_context);

    // Get the direction from the property binding
    let direction = flexbox_layout_direction(flexbox_layout, local_context);

    // Determine if we're on the main axis or cross axis
    let is_main_axis = matches!(
        (direction, orientation),
        (FlexDirection::Row | FlexDirection::RowReverse, Orientation::Horizontal)
            | (FlexDirection::Column | FlexDirection::ColumnReverse, Orientation::Vertical)
    );

    let (padding_h, spacing_h) =
        padding_and_spacing(&flexbox_layout.geometry, Orientation::Horizontal, &expr_eval);
    let (padding_v, spacing_v) =
        padding_and_spacing(&flexbox_layout.geometry, Orientation::Vertical, &expr_eval);

    if is_main_axis {
        // Main axis: use simple layout info (no constraint needed)
        // This avoids reading the perpendicular dimension and prevents circular dependencies
        core_layout::flexbox_layout_info(
            Slice::from(cells_h.as_slice()),
            Slice::from(cells_v.as_slice()),
            spacing_h,
            spacing_v,
            &padding_h,
            &padding_v,
            to_runtime(orientation),
            direction,
            Coord::MAX,
        )
        .into()
    } else {
        // Cross axis: need constraint to handle wrapping
        // Only read the constraint dimension here (the main-axis dimension of the flexbox)
        let constraint_size = match orientation {
            Orientation::Horizontal => {
                // Cross-axis for Column: need height
                let height_ref = &flexbox_layout.geometry.rect.height_reference;
                height_ref.as_ref().map(&expr_eval).unwrap_or(0.)
            }
            Orientation::Vertical => {
                // Cross-axis for Row: need width
                let width_ref = &flexbox_layout.geometry.rect.width_reference;
                width_ref.as_ref().map(&expr_eval).unwrap_or(0.)
            }
        };

        core_layout::flexbox_layout_info(
            Slice::from(cells_h.as_slice()),
            Slice::from(cells_v.as_slice()),
            spacing_h,
            spacing_v,
            &padding_h,
            &padding_v,
            to_runtime(orientation),
            direction,
            constraint_size,
        )
        .into()
    }
}

fn flexbox_layout_data(
    flexbox_layout: &i_slint_compiler::layout::FlexBoxLayout,
    component: InstanceRef,
    expr_eval: &impl Fn(&NamedReference) -> f32,
    _local_context: &mut EvalLocalContext,
) -> (Vec<core_layout::LayoutItemInfo>, Vec<core_layout::LayoutItemInfo>, Vec<u32>) {
    let window_adapter = component.window_adapter();
    let mut cells_h = Vec::with_capacity(flexbox_layout.elems.len());
    let mut cells_v = Vec::with_capacity(flexbox_layout.elems.len());
    let mut repeated_indices = Vec::new();

    for layout_elem in &flexbox_layout.elems {
        if layout_elem.element.borrow().repeated.is_some() {
            let component_vec = repeater_instances(component, &layout_elem.element);
            repeated_indices.push(cells_h.len() as u32);
            repeated_indices.push(component_vec.len() as u32);
            cells_h.extend(component_vec.iter().map(|x| {
                x.as_pin_ref().layout_item_info(to_runtime(Orientation::Horizontal), None)
            }));
            cells_v.extend(
                component_vec.iter().map(|x| {
                    x.as_pin_ref().layout_item_info(to_runtime(Orientation::Vertical), None)
                }),
            );
        } else {
            let mut layout_info_h = get_layout_info(
                &layout_elem.element,
                component,
                &window_adapter,
                Orientation::Horizontal,
            );
            fill_layout_info_constraints(
                &mut layout_info_h,
                &layout_elem.constraints,
                Orientation::Horizontal,
                &expr_eval,
            );
            cells_h.push(core_layout::LayoutItemInfo { constraint: layout_info_h });

            let mut layout_info_v = get_layout_info(
                &layout_elem.element,
                component,
                &window_adapter,
                Orientation::Vertical,
            );
            fill_layout_info_constraints(
                &mut layout_info_v,
                &layout_elem.constraints,
                Orientation::Vertical,
                &expr_eval,
            );
            cells_v.push(core_layout::LayoutItemInfo { constraint: layout_info_v });
        }
    }

    (cells_h, cells_v, repeated_indices)
}

fn padding_and_spacing(
    layout_geometry: &LayoutGeometry,
    orientation: Orientation,
    expr_eval: &impl Fn(&NamedReference) -> f32,
) -> (core_layout::Padding, f32) {
    let spacing = layout_geometry.spacing.orientation(orientation).map_or(0., expr_eval);
    let (begin, end) = layout_geometry.padding.begin_end(orientation);
    let padding =
        core_layout::Padding { begin: begin.map_or(0., expr_eval), end: end.map_or(0., expr_eval) };
    (padding, spacing)
}

fn repeater_instances(
    component: InstanceRef,
    elem: &ElementRc,
) -> Vec<crate::dynamic_item_tree::DynamicComponentVRc> {
    generativity::make_guard!(guard);
    let rep =
        crate::dynamic_item_tree::get_repeater_by_name(component, elem.borrow().id.as_str(), guard);
    let extra_data = component.description.extra_data_offset.apply(component.as_ref());
    rep.0.as_ref().ensure_updated(|| {
        crate::dynamic_item_tree::instantiate(
            rep.1.clone(),
            component.self_weak().get().cloned(),
            None,
            None,
            extra_data.globals.get().unwrap().clone(),
        )
    });
    rep.0.as_ref().instances_vec()
}

fn grid_layout_input_data(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    ctx: &EvalLocalContext,
) -> Vec<core_layout::GridLayoutInputData> {
    let component = ctx.component_instance;
    let mut result = Vec::with_capacity(grid_layout.elems.len());
    let mut after_repeater_in_same_row = false;
    let mut new_row = true;
    for elem in grid_layout.elems.iter() {
        let eval_or_default = |expr: &RowColExpr, component: InstanceRef| match expr {
            RowColExpr::Literal(value) => *value as f32,
            RowColExpr::Auto => i_slint_common::ROW_COL_AUTO,
            RowColExpr::Named(nr) => {
                // we could check for out-of-bounds here, but organize_grid_layout will also do it
                eval::load_property(component, &nr.element(), nr.name())
                    .unwrap()
                    .try_into()
                    .unwrap()
            }
        };

        let cell_new_row = elem.cell.borrow().new_row;
        if cell_new_row {
            after_repeater_in_same_row = false;
        }
        if elem.item.element.borrow().repeated.is_some() {
            let component_vec = repeater_instances(component, &elem.item.element);
            new_row = cell_new_row;
            for erased_sub_comp in &component_vec {
                // Evaluate the row/col/rowspan/colspan expressions in the context of the sub-component
                generativity::make_guard!(guard);
                let sub_comp = erased_sub_comp.as_pin_ref();
                let sub_instance_ref =
                    unsafe { InstanceRef::from_pin_ref(sub_comp.borrow(), guard) };

                let mut push_cell = |cell: &i_slint_compiler::layout::GridLayoutCell,
                                     new_row: bool| {
                    let row = eval_or_default(&cell.row_expr, sub_instance_ref);
                    let col = eval_or_default(&cell.col_expr, sub_instance_ref);
                    let rowspan = eval_or_default(&cell.rowspan_expr, sub_instance_ref);
                    let colspan = eval_or_default(&cell.colspan_expr, sub_instance_ref);

                    result.push(core_layout::GridLayoutInputData {
                        new_row,
                        col,
                        row,
                        colspan,
                        rowspan,
                    });
                };

                if let Some(children) = elem.cell.borrow().child_items.as_ref() {
                    // Repeated row
                    new_row = true;
                    for child_item in children {
                        let element_ref = &child_item.element.borrow();
                        let child_cell = element_ref.grid_layout_cell.as_ref().unwrap().borrow();
                        push_cell(&child_cell, new_row);
                        new_row = false;
                    }
                } else {
                    // Single repeated item
                    let cell = elem.cell.borrow();
                    push_cell(&cell, new_row);
                    new_row = false;
                }
            }
            after_repeater_in_same_row = true;
        } else {
            let new_row =
                if cell_new_row || !after_repeater_in_same_row { cell_new_row } else { new_row };
            let row = eval_or_default(&elem.cell.borrow().row_expr, component);
            let col = eval_or_default(&elem.cell.borrow().col_expr, component);
            let rowspan = eval_or_default(&elem.cell.borrow().rowspan_expr, component);
            let colspan = eval_or_default(&elem.cell.borrow().colspan_expr, component);
            result.push(core_layout::GridLayoutInputData { new_row, col, row, colspan, rowspan });
        }
    }
    result
}

fn grid_repeater_indices(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    ctx: &mut EvalLocalContext,
) -> Vec<u32> {
    let component = ctx.component_instance;
    let mut repeater_indices = Vec::new();

    let mut num_cells = 0;
    for elem in grid_layout.elems.iter() {
        if elem.item.element.borrow().repeated.is_some() {
            let component_vec = repeater_instances(component, &elem.item.element);
            repeater_indices.push(num_cells as _);
            repeater_indices.push(component_vec.len() as _);
            let item_count = elem.cell.borrow().child_items.as_ref().map_or(1, |c| c.len());
            num_cells += component_vec.len() * item_count;
        } else {
            num_cells += 1;
        }
    }
    repeater_indices
}

fn grid_repeater_steps(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    _ctx: &mut EvalLocalContext,
) -> Vec<u32> {
    let mut repeater_steps = Vec::new();
    for elem in grid_layout.elems.iter() {
        if elem.item.element.borrow().repeated.is_some() {
            let item_count = elem.cell.borrow().child_items.as_ref().map_or(1, |c| c.len());
            repeater_steps.push(item_count as u32);
        }
    }
    repeater_steps
}

fn grid_layout_constraints(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    orientation: Orientation,
    ctx: &mut EvalLocalContext,
) -> Vec<core_layout::LayoutItemInfo> {
    let component = ctx.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    let mut constraints = Vec::with_capacity(grid_layout.elems.len());

    for layout_elem in grid_layout.elems.iter() {
        if layout_elem.item.element.borrow().repeated.is_some() {
            let component_vec = repeater_instances(component, &layout_elem.item.element);
            let child_items = layout_elem.cell.borrow().child_items.clone();
            let repeated_children_count = child_items.as_ref().map(|c| c.len());
            if let Some(num) = repeated_children_count {
                // Repeated row
                for sub_comp in &component_vec {
                    // Evaluate constraints in the context of the repeated sub-component
                    generativity::make_guard!(guard);
                    let sub_pin = sub_comp.as_pin_ref();
                    let sub_borrow = sub_pin.borrow();
                    let sub_instance_ref = unsafe { InstanceRef::from_pin_ref(sub_borrow, guard) };
                    let expr_eval = |nr: &NamedReference| -> f32 {
                        eval::load_property(sub_instance_ref, &nr.element(), nr.name())
                            .unwrap()
                            .try_into()
                            .unwrap()
                    };
                    for idx in 0..num {
                        let mut layout_info =
                            sub_pin.layout_item_info(to_runtime(orientation), Some(idx));
                        if let Some(child_item) = child_items.as_ref().and_then(|cc| cc.get(idx)) {
                            fill_layout_info_constraints(
                                &mut layout_info.constraint,
                                &child_item.constraints,
                                orientation,
                                &expr_eval,
                            );
                        }
                        constraints.push(layout_info);
                    }
                }
            } else {
                // Single repeated item
                constraints.extend(
                    component_vec
                        .iter()
                        .map(|x| x.as_pin_ref().layout_item_info(to_runtime(orientation), None)),
                );
            }
        } else {
            let mut layout_info = get_layout_info(
                &layout_elem.item.element,
                component,
                &component.window_adapter(),
                orientation,
            );
            fill_layout_info_constraints(
                &mut layout_info,
                &layout_elem.item.constraints,
                orientation,
                &expr_eval,
            );
            constraints.push(core_layout::LayoutItemInfo { constraint: layout_info });
        }
    }
    constraints
}

fn box_layout_data(
    box_layout: &i_slint_compiler::layout::BoxLayout,
    orientation: Orientation,
    component: InstanceRef,
    expr_eval: &impl Fn(&NamedReference) -> f32,
    mut repeater_indices: Option<&mut Vec<u32>>,
) -> (Vec<core_layout::LayoutItemInfo>, i_slint_core::items::LayoutAlignment) {
    let window_adapter = component.window_adapter();
    let mut cells = Vec::with_capacity(box_layout.elems.len());
    for cell in &box_layout.elems {
        if cell.element.borrow().repeated.is_some() {
            let component_vec = repeater_instances(component, &cell.element);
            if let Some(ri) = repeater_indices.as_mut() {
                ri.push(cells.len() as _);
                ri.push(component_vec.len() as _);
            }
            cells.extend(
                component_vec
                    .iter()
                    .map(|x| x.as_pin_ref().layout_item_info(to_runtime(orientation), None)),
            );
        } else {
            let mut layout_info =
                get_layout_info(&cell.element, component, &window_adapter, orientation);
            fill_layout_info_constraints(
                &mut layout_info,
                &cell.constraints,
                orientation,
                &expr_eval,
            );
            cells.push(core_layout::LayoutItemInfo { constraint: layout_info });
        }
    }
    let alignment = box_layout
        .geometry
        .alignment
        .as_ref()
        .map(|nr| {
            eval::load_property(component, &nr.element(), nr.name())
                .unwrap()
                .try_into()
                .unwrap_or_default()
        })
        .unwrap_or_default();
    (cells, alignment)
}

pub(crate) fn fill_layout_info_constraints(
    layout_info: &mut core_layout::LayoutInfo,
    constraints: &LayoutConstraints,
    orientation: Orientation,
    expr_eval: &impl Fn(&NamedReference) -> f32,
) {
    let is_percent =
        |nr: &NamedReference| Expression::PropertyReference(nr.clone()).ty() == Type::Percent;

    match orientation {
        Orientation::Horizontal => {
            if let Some(e) = constraints.min_width.as_ref() {
                if !is_percent(e) {
                    layout_info.min = expr_eval(e)
                } else {
                    layout_info.min_percent = expr_eval(e)
                }
            }
            if let Some(e) = constraints.max_width.as_ref() {
                if !is_percent(e) {
                    layout_info.max = expr_eval(e)
                } else {
                    layout_info.max_percent = expr_eval(e)
                }
            }
            if let Some(e) = constraints.preferred_width.as_ref() {
                layout_info.preferred = expr_eval(e);
            }
            if let Some(e) = constraints.horizontal_stretch.as_ref() {
                layout_info.stretch = expr_eval(e);
            }
        }
        Orientation::Vertical => {
            if let Some(e) = constraints.min_height.as_ref() {
                if !is_percent(e) {
                    layout_info.min = expr_eval(e)
                } else {
                    layout_info.min_percent = expr_eval(e)
                }
            }
            if let Some(e) = constraints.max_height.as_ref() {
                if !is_percent(e) {
                    layout_info.max = expr_eval(e)
                } else {
                    layout_info.max_percent = expr_eval(e)
                }
            }
            if let Some(e) = constraints.preferred_height.as_ref() {
                layout_info.preferred = expr_eval(e);
            }
            if let Some(e) = constraints.vertical_stretch.as_ref() {
                layout_info.stretch = expr_eval(e);
            }
        }
    }
}

/// Get the layout info for an element based on the layout_info_prop or the builtin item layout_info
pub(crate) fn get_layout_info(
    elem: &ElementRc,
    component: InstanceRef,
    window_adapter: &Rc<dyn WindowAdapter>,
    orientation: Orientation,
) -> core_layout::LayoutInfo {
    let elem = elem.borrow();
    if let Some(nr) = elem.layout_info_prop(orientation) {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    } else {
        let item = &component
            .description
            .items
            .get(elem.id.as_str())
            .unwrap_or_else(|| panic!("Internal error: Item {} not found", elem.id));
        let item_comp = component.self_weak().get().unwrap().upgrade().unwrap();

        unsafe {
            item.item_from_item_tree(component.as_ptr()).as_ref().layout_info(
                to_runtime(orientation),
                window_adapter,
                &ItemRc::new(vtable::VRc::into_dyn(item_comp), item.item_index()),
            )
        }
    }
}
