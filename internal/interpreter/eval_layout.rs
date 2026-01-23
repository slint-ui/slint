// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Value;
use crate::dynamic_item_tree::InstanceRef;
use crate::eval::{self, EvalLocalContext};
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::Type;
use i_slint_compiler::layout::{
    GridLayout, Layout, LayoutConstraints, LayoutGeometry, Orientation, RowColExpr,
};
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_core::items::{DialogButtonRole, ItemRc};
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

pub(crate) fn compute_layout_info(
    lay: &Layout,
    orientation: Orientation,
    local_context: &mut EvalLocalContext,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    match lay {
        Layout::GridLayout(_) => {
            panic!("only BoxLayout is supported");
        }
        Layout::BoxLayout(box_layout) => {
            let (cells, alignment) =
                box_layout_data(box_layout, orientation, component, &expr_eval, None);
            let (padding, spacing) =
                padding_and_spacing(&box_layout.geometry, orientation, &expr_eval);
            if orientation == box_layout.orientation {
                core_layout::box_layout_info(
                    Slice::from(cells.as_slice()),
                    spacing,
                    &padding,
                    alignment,
                )
            } else {
                core_layout::box_layout_info_ortho(Slice::from(cells.as_slice()), &padding)
            }
            .into()
        }
    }
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

pub(crate) fn solve_layout(
    lay: &Layout,
    orientation: Orientation,
    local_context: &mut EvalLocalContext,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };

    match lay {
        Layout::GridLayout(_) => {
            panic!("solve_layout called on GridLayout; use solve_grid_layout instead");
        }
        Layout::BoxLayout(box_layout) => {
            let mut repeated_indices = Vec::new();
            let (cells, alignment) = box_layout_data(
                box_layout,
                orientation,
                component,
                &expr_eval,
                Some(&mut repeated_indices),
            );
            let (padding, spacing) =
                padding_and_spacing(&box_layout.geometry, orientation, &expr_eval);
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
    }
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
                let row = eval_or_default(&elem.cell.borrow().row_expr, sub_instance_ref);
                let col = eval_or_default(&elem.cell.borrow().col_expr, sub_instance_ref);
                let rowspan = eval_or_default(&elem.cell.borrow().rowspan_expr, sub_instance_ref);
                let colspan = eval_or_default(&elem.cell.borrow().colspan_expr, sub_instance_ref);
                let repeated_children_count =
                    elem.cell.borrow().child_items.as_ref().map(|c| c.len());
                if repeated_children_count.is_some() {
                    new_row = true;
                }
                for _ in 0..repeated_children_count.unwrap_or(1) {
                    result.push(core_layout::GridLayoutInputData {
                        new_row,
                        col,
                        row,
                        colspan,
                        rowspan,
                    });
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
