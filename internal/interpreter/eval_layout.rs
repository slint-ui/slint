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
    let constraints = grid_layout_constraints(grid_layout, orientation, local_context);
    core_layout::grid_layout_info(
        organized_data.clone(),
        Slice::from_slice(constraints.as_slice()),
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
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    let cells = grid_layout_input_data(layout, &expr_eval);

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
        core_layout::organize_grid_layout(Slice::from_slice(cells.as_slice())).into()
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

fn grid_layout_input_data(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    expr_eval: &impl Fn(&NamedReference) -> f32,
) -> Vec<core_layout::GridLayoutInputData> {
    grid_layout
        .elems
        .iter()
        .map(|cell| {
            let eval_or_default = |expr: &RowColExpr| match expr {
                RowColExpr::Literal(value) => *value,
                RowColExpr::Named(e) => {
                    let value = expr_eval(e);
                    if value >= 0.0 && value <= u16::MAX as f32 {
                        value as u16
                    } else {
                        panic!(
                            "Expected a positive integer, but got {:?} while evaluating {:?}",
                            value, e
                        );
                    }
                }
            };
            let row = eval_or_default(&cell.row_expr);
            let col = eval_or_default(&cell.col_expr);
            let rowspan = eval_or_default(&cell.rowspan_expr);
            let colspan = eval_or_default(&cell.colspan_expr);
            core_layout::GridLayoutInputData { new_row: cell.new_row, col, row, colspan, rowspan }
        })
        .collect()
}

fn grid_layout_constraints(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    orientation: Orientation,
    ctx: &mut EvalLocalContext,
) -> Vec<core_layout::LayoutInfo> {
    let component = ctx.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    let mut constraints = Vec::with_capacity(grid_layout.elems.len());

    for cell in grid_layout.elems.iter() {
        let mut layout_info = get_layout_info(
            &cell.item.element,
            component,
            &component.window_adapter(),
            orientation,
        );
        fill_layout_info_constraints(
            &mut layout_info,
            &cell.item.constraints,
            orientation,
            &expr_eval,
        );
        constraints.push(layout_info);
    }
    constraints
}

fn box_layout_data(
    box_layout: &i_slint_compiler::layout::BoxLayout,
    orientation: Orientation,
    component: InstanceRef,
    expr_eval: &impl Fn(&NamedReference) -> f32,
    mut repeater_indices: Option<&mut Vec<u32>>,
) -> (Vec<core_layout::BoxLayoutCellData>, i_slint_core::items::LayoutAlignment) {
    let window_adapter = component.window_adapter();
    let mut cells = Vec::with_capacity(box_layout.elems.len());
    for cell in &box_layout.elems {
        if cell.element.borrow().repeated.is_some() {
            generativity::make_guard!(guard);
            let rep = crate::dynamic_item_tree::get_repeater_by_name(
                component,
                cell.element.borrow().id.as_str(),
                guard,
            );
            rep.0.as_ref().ensure_updated(|| {
                let instance = crate::dynamic_item_tree::instantiate(
                    rep.1.clone(),
                    component.self_weak().get().cloned(),
                    None,
                    None,
                    Default::default(),
                );
                instance
            });
            let component_vec = rep.0.as_ref().instances_vec();
            if let Some(ri) = repeater_indices.as_mut() {
                ri.push(cells.len() as _);
                ri.push(component_vec.len() as _);
            }
            cells.extend(
                component_vec
                    .iter()
                    .map(|x| x.as_pin_ref().box_layout_data(to_runtime(orientation))),
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
            cells.push(core_layout::BoxLayoutCellData { constraint: layout_info });
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
