// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::dynamic_item_tree::InstanceRef;
use crate::eval::{self, EvalLocalContext};
use crate::Value;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::Type;
use i_slint_compiler::layout::{Layout, LayoutConstraints, LayoutGeometry, Orientation};
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_core::items::DialogButtonRole;
use i_slint_core::layout::{self as core_layout};
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
        Layout::GridLayout(grid_layout) => {
            let cells = grid_layout_data(grid_layout, orientation, component, &expr_eval);
            let (padding, spacing) =
                padding_and_spacing(&grid_layout.geometry, orientation, &expr_eval);
            core_layout::grid_layout_info(Slice::from(cells.as_slice()), spacing, &padding).into()
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
        Layout::GridLayout(grid_layout) => {
            let mut cells = grid_layout_data(grid_layout, orientation, component, &expr_eval);
            if let (Some(buttons_roles), Orientation::Horizontal) =
                (&grid_layout.dialog_button_roles, orientation)
            {
                let roles = buttons_roles
                    .iter()
                    .map(|r| DialogButtonRole::from_str(r).unwrap())
                    .collect::<Vec<_>>();
                core_layout::reorder_dialog_button_layout(&mut cells, &roles);
            }

            let (padding, spacing) =
                padding_and_spacing(&grid_layout.geometry, orientation, &expr_eval);

            let size_ref = grid_layout.geometry.rect.size_reference(orientation);
            core_layout::solve_grid_layout(&core_layout::GridLayoutData {
                size: size_ref.map(expr_eval).unwrap_or(0.),
                spacing,
                padding,
                cells: Slice::from(cells.as_slice()),
            })
            .into()
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

/// return the celldata, the padding, and the spacing of a grid layout
fn grid_layout_data(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    orientation: Orientation,
    component: InstanceRef,
    expr_eval: &impl Fn(&NamedReference) -> f32,
) -> Vec<core_layout::GridLayoutCellData> {
    let cells = grid_layout
        .elems
        .iter()
        .map(|cell| {
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
            let (col_or_row, span) = cell.col_or_row_and_span(orientation);
            core_layout::GridLayoutCellData { col_or_row, span, constraint: layout_info }
        })
        .collect::<Vec<_>>();
    cells
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
        unsafe {
            item.item_from_item_tree(component.as_ptr())
                .as_ref()
                .layout_info(to_runtime(orientation), window_adapter)
        }
    }
}
