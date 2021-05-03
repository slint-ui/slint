/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use crate::dynamic_component::InstanceRef;
use crate::eval::{self, ComponentInstance, EvalLocalContext};
use crate::Value;
use sixtyfps_compilerlib::expression_tree::Expression;
use sixtyfps_compilerlib::langtype::Type;
use sixtyfps_compilerlib::layout::{Layout, LayoutConstraints, LayoutItem};
use sixtyfps_compilerlib::namedreference::NamedReference;
use sixtyfps_corelib::layout as core_layout;
use sixtyfps_corelib::model::RepeatedComponent;
use sixtyfps_corelib::slice::Slice;
use sixtyfps_corelib::window::ComponentWindow;
use std::convert::TryInto;

pub(crate) fn compute_layout_info(lay: &Layout, local_context: &mut EvalLocalContext) -> Value {
    let component = match local_context.component_instance {
        ComponentInstance::InstanceRef(c) => c,
        ComponentInstance::GlobalComponent(_) => panic!("Cannot compute layout from a Global"),
    };
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    match lay {
        Layout::GridLayout(grid_layout) => {
            let (cells, padding, spacing) = grid_layout_data(grid_layout, component, &expr_eval);
            core_layout::grid_layout_info(&Slice::from(cells.as_slice()), spacing, &padding).into()
        }
        Layout::BoxLayout(box_layout) => {
            let (cells, padding, spacing, alignment) =
                box_layout_data(box_layout, component, &expr_eval, None);
            core_layout::box_layout_info(
                &Slice::from(cells.as_slice()),
                spacing,
                &padding,
                alignment,
                box_layout.is_horizontal,
            )
            .into()
        }
        _ => todo!(),
    }
}

pub(crate) fn solve_layout(lay: &Layout, local_context: &mut EvalLocalContext) -> Value {
    let component = match local_context.component_instance {
        ComponentInstance::InstanceRef(c) => c,
        ComponentInstance::GlobalComponent(_) => panic!("Cannot compute layout from a Global"),
    };
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };

    match lay {
        Layout::GridLayout(grid_layout) => {
            let (cells, padding, spacing) = grid_layout_data(grid_layout, component, &expr_eval);

            Value::LayoutCache(core_layout::solve_grid_layout(&core_layout::GridLayoutData {
                width: grid_layout
                    .geometry
                    .rect
                    .width_reference
                    .as_ref()
                    .map(expr_eval)
                    .unwrap_or(0.),
                height: grid_layout
                    .geometry
                    .rect
                    .height_reference
                    .as_ref()
                    .map(expr_eval)
                    .unwrap_or(0.),
                x: 0.,
                y: 0.,
                spacing,
                padding: &padding,
                cells: Slice::from(cells.as_slice()),
            }))
        }
        Layout::BoxLayout(box_layout) => {
            let mut repeated_indices = Vec::new();
            let (cells, padding, spacing, alignment) =
                box_layout_data(box_layout, component, &expr_eval, Some(&mut repeated_indices));
            core_layout::solve_box_layout(
                &core_layout::BoxLayoutData {
                    width: box_layout
                        .geometry
                        .rect
                        .width_reference
                        .as_ref()
                        .map(expr_eval)
                        .unwrap_or(0.),
                    height: box_layout
                        .geometry
                        .rect
                        .height_reference
                        .as_ref()
                        .map(expr_eval)
                        .unwrap_or(0.),
                    x: 0.,
                    y: 0.,
                    spacing,
                    padding: &padding,
                    alignment,
                    cells: Slice::from(cells.as_slice()),
                },
                box_layout.is_horizontal,
                Slice::from(repeated_indices.as_slice()),
            )
            .into()
        }
        _ => todo!(),
    }
}

/// return the celldata, the padding, and the spacing of a grid layout
fn grid_layout_data(
    grid_layout: &sixtyfps_compilerlib::layout::GridLayout,
    component: InstanceRef,
    expr_eval: &impl Fn(&NamedReference) -> f32,
) -> (Vec<core_layout::GridLayoutCellData>, core_layout::Padding, f32) {
    let cells = grid_layout
        .elems
        .iter()
        .map(|cell| {
            let mut layout_info =
                get_layout_info(&cell.item, component, &eval::window_ref(component).unwrap());
            fill_layout_info_constraints(&mut layout_info, &cell.item.constraints, &expr_eval);
            core_layout::GridLayoutCellData {
                col: cell.col,
                row: cell.row,
                colspan: cell.colspan,
                rowspan: cell.rowspan,
                constraint: layout_info,
            }
        })
        .collect::<Vec<_>>();
    let spacing = grid_layout.geometry.spacing.as_ref().map_or(0., expr_eval);
    let padding = core_layout::Padding {
        left: grid_layout.geometry.padding.left.as_ref().map_or(0., expr_eval),
        right: grid_layout.geometry.padding.right.as_ref().map_or(0., expr_eval),
        top: grid_layout.geometry.padding.top.as_ref().map_or(0., expr_eval),
        bottom: grid_layout.geometry.padding.bottom.as_ref().map_or(0., expr_eval),
    };
    (cells, padding, spacing)
}

fn box_layout_data(
    box_layout: &sixtyfps_compilerlib::layout::BoxLayout,
    component: InstanceRef,
    expr_eval: &impl Fn(&NamedReference) -> f32,
    mut repeater_indices: Option<&mut Vec<u32>>,
) -> (Vec<core_layout::BoxLayoutCellData>, core_layout::Padding, f32, core_layout::LayoutAlignment)
{
    let window = eval::window_ref(component).unwrap();
    let mut cells = Vec::with_capacity(box_layout.elems.len());
    for cell in &box_layout.elems {
        if cell.element.borrow().repeated.is_some() {
            generativity::make_guard!(guard);
            let rep = crate::dynamic_component::get_repeater_by_name(
                component,
                cell.element.borrow().id.as_str(),
                guard,
            );
            rep.0.as_ref().ensure_updated(|| {
                let instance = crate::dynamic_component::instantiate(
                    rep.1.clone(),
                    Some(component.borrow()),
                    window.clone(),
                );
                instance.run_setup_code();
                instance
            });
            let component_vec = rep.0.as_ref().components_vec();
            if let Some(ri) = repeater_indices.as_mut() {
                ri.push(cells.len() as _);
                ri.push(component_vec.len() as _);
            }
            cells.extend(component_vec.iter().map(|x| x.as_pin_ref().box_layout_data()));
        } else {
            let mut layout_info = get_layout_info(cell, component, &window);
            fill_layout_info_constraints(&mut layout_info, &cell.constraints, &expr_eval);
            cells.push(core_layout::BoxLayoutCellData { constraint: layout_info });
        }
    }
    let spacing = box_layout.geometry.spacing.as_ref().map_or(0., expr_eval);
    let padding = core_layout::Padding {
        left: box_layout.geometry.padding.left.as_ref().map_or(0., expr_eval),
        right: box_layout.geometry.padding.right.as_ref().map_or(0., expr_eval),
        top: box_layout.geometry.padding.top.as_ref().map_or(0., expr_eval),
        bottom: box_layout.geometry.padding.bottom.as_ref().map_or(0., expr_eval),
    };
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
    (cells, padding, spacing, alignment)
}

pub(crate) fn fill_layout_info_constraints(
    layout_info: &mut core_layout::LayoutInfo,
    constraints: &LayoutConstraints,
    expr_eval: &impl Fn(&NamedReference) -> f32,
) {
    let is_percent =
        |nr: &NamedReference| Expression::PropertyReference(nr.clone()).ty() == Type::Percent;
    constraints.minimum_width.as_ref().map(|e| {
        if !is_percent(e) {
            layout_info.min_width = expr_eval(e)
        } else {
            layout_info.min_width_percent = expr_eval(e)
        }
    });
    constraints.maximum_width.as_ref().map(|e| {
        if !is_percent(e) {
            layout_info.max_width = expr_eval(e)
        } else {
            layout_info.max_width_percent = expr_eval(e)
        }
    });
    constraints.minimum_height.as_ref().map(|e| {
        if !is_percent(e) {
            layout_info.min_height = expr_eval(e)
        } else {
            layout_info.min_height_percent = expr_eval(e)
        }
    });
    constraints.maximum_height.as_ref().map(|e| {
        if !is_percent(e) {
            layout_info.max_height = expr_eval(e)
        } else {
            layout_info.max_height_percent = expr_eval(e)
        }
    });
    constraints.preferred_width.as_ref().map(|e| {
        layout_info.preferred_width = expr_eval(e);
    });
    constraints.preferred_height.as_ref().map(|e| {
        layout_info.preferred_height = expr_eval(e);
    });
    constraints.horizontal_stretch.as_ref().map(|e| layout_info.horizontal_stretch = expr_eval(e));
    constraints.vertical_stretch.as_ref().map(|e| layout_info.vertical_stretch = expr_eval(e));
}

fn get_layout_info<'a, 'b>(
    item: &'a LayoutItem,
    component: InstanceRef<'a, '_>,
    window: &ComponentWindow,
) -> core_layout::LayoutInfo {
    let elem = item.element.borrow();
    let item = &component
        .component_type
        .items
        .get(elem.id.as_str())
        .unwrap_or_else(|| panic!("Internal error: Item {} not found", elem.id));
    let r = unsafe { item.item_from_component(component.as_ptr()).as_ref().layouting_info(window) };
    if let Some(nr) = &elem.layout_info_prop {
        r.merge(
            &eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap(),
        )
    } else {
        r
    }
}
