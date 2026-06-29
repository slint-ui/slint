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
use i_slint_core::items::{DialogButtonRole, FlexboxLayoutDirection, ItemRc};
use i_slint_core::layout::{self as core_layout, GridLayoutInputData, GridLayoutOrganizedData};
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
    cross_axis_size: Option<f32>,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    let (padding, spacing) = padding_and_spacing(&grid_layout.geometry, orientation, &expr_eval);
    let repeater_steps = grid_repeater_steps(grid_layout, local_context);
    let repeater_indices = grid_repeater_indices(grid_layout, local_context, &repeater_steps);
    let constraints = grid_layout_constraints(
        grid_layout,
        orientation,
        local_context,
        &repeater_steps,
        cross_axis_size,
    );
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

/// Determine layout info of a box layout
pub(crate) fn compute_box_layout_info(
    box_layout: &BoxLayout,
    orientation: Orientation,
    local_context: &mut EvalLocalContext,
    cross_axis_size: Option<f32>,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    let cross_axis_size = cross_axis_size.map(|w| {
        let (cross_pad, _) =
            padding_and_spacing(&box_layout.geometry, orientation.orthogonal(), &expr_eval);
        w - cross_pad.begin - cross_pad.end
    });
    let (cells, alignment) = box_layout_data(
        box_layout,
        orientation,
        component,
        &expr_eval,
        None,
        cross_axis_size,
        local_context,
        None,
    );
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
    let repeater_steps = grid_repeater_steps(layout, local_context);
    let cells = grid_layout_input_data(layout, local_context, &repeater_steps);
    let repeater_indices = grid_repeater_indices(layout, local_context, &repeater_steps);
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
    let repeater_steps = grid_repeater_steps(grid_layout, local_context);
    let repeater_indices = grid_repeater_indices(grid_layout, local_context, &repeater_steps);
    let constraints =
        grid_layout_constraints(grid_layout, orientation, local_context, &repeater_steps, None);

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
    // For a horizontal layout's main (width) pass, supply the layout's real cross
    // size (content height) so width-for-height children (e.g. a column
    // FlexboxLayout) compute their width from it via layoutinfo-h-with-constraint,
    // instead of reading self.height and cycling through our own vertical pass.
    let cross_axis_size = (orientation == box_layout.orientation
        && orientation == Orientation::Horizontal
        && box_layout
            .elems
            .iter()
            .any(|c| c.element.borrow().inherited_layout_info_h_with_constraint().is_some()))
    .then(|| {
        let cross = orientation.orthogonal();
        box_layout.geometry.rect.size_reference(cross).map(&expr_eval).map(|s| {
            let (pad, _) = padding_and_spacing(&box_layout.geometry, cross, &expr_eval);
            s - pad.begin - pad.end
        })
    })
    .flatten();
    // On the cross pass, the layout's real cross size (its own content size
    // along this orientation) is known. Forward it so a wrapping perpendicular
    // flex cell can be given its natural single-line size instead of the
    // compact sqrt preferred (see `clamp_wrapping_flex_cross_preferred`).
    let available_cross = (orientation != box_layout.orientation)
        .then(|| {
            box_layout.geometry.rect.size_reference(orientation).map(&expr_eval).map(|s| {
                let (pad, _) = padding_and_spacing(&box_layout.geometry, orientation, &expr_eval);
                s - pad.begin - pad.end
            })
        })
        .flatten();
    let (cells, alignment) = box_layout_data(
        box_layout,
        orientation,
        component,
        &expr_eval,
        Some(&mut repeated_indices),
        cross_axis_size,
        local_context,
        available_cross,
    );
    let (padding, spacing) = padding_and_spacing(&box_layout.geometry, orientation, &expr_eval);
    let size = box_layout.geometry.rect.size_reference(orientation).map(&expr_eval).unwrap_or(0.);
    if orientation == box_layout.orientation {
        core_layout::solve_box_layout(
            &core_layout::BoxLayoutData {
                size,
                spacing,
                padding,
                alignment,
                cells: Slice::from(cells.as_slice()),
            },
            Slice::from(repeated_indices.as_slice()),
        )
        .into()
    } else {
        let cross_axis_alignment = box_layout
            .cross_alignment
            .as_ref()
            .map(|nr| {
                eval::load_property(component, &nr.element(), nr.name())
                    .unwrap()
                    .try_into()
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        core_layout::solve_box_layout_ortho(
            &core_layout::BoxLayoutOrthoData {
                size,
                padding,
                cross_axis_alignment,
                cells: Slice::from(cells.as_slice()),
            },
            Slice::from(repeated_indices.as_slice()),
        )
        .into()
    }
}

pub(crate) fn solve_flexbox_layout(
    flexbox_layout: &i_slint_compiler::layout::FlexboxLayout,
    local_context: &mut EvalLocalContext,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };

    let width_ref = &flexbox_layout.geometry.rect.width_reference;
    let height_ref = &flexbox_layout.geometry.rect.height_reference;
    let direction = flexbox_layout_direction(flexbox_layout, local_context);

    // For column direction, pass the container content width (outer width minus
    // horizontal padding) so cells_v can use it as the constraint for
    // height-for-width items — the width they are actually laid out at.
    let container_width_for_cells = match direction {
        i_slint_core::items::FlexboxLayoutDirection::Column
        | i_slint_core::items::FlexboxLayoutDirection::ColumnReverse => {
            width_ref.as_ref().map(|w| {
                let (pad_h, _) = padding_and_spacing(
                    &flexbox_layout.geometry,
                    Orientation::Horizontal,
                    &expr_eval,
                );
                expr_eval(w) - pad_h.begin - pad_h.end
            })
        }
        _ => None,
    };

    let (cells_h, cells_v, repeated_indices) = flexbox_layout_data(
        flexbox_layout,
        component,
        &expr_eval,
        local_context,
        container_width_for_cells,
        None,
    );

    let alignment = flexbox_layout
        .geometry
        .alignment
        .as_ref()
        .map_or(i_slint_core::items::LayoutAlignment::default(), |nr| {
            eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
        });
    let align_content = flexbox_layout
        .align_content
        .as_ref()
        .map_or(i_slint_core::items::FlexboxLayoutAlignContent::default(), |nr| {
            eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
        });
    let cross_axis_alignment = flexbox_layout
        .cross_axis_alignment
        .as_ref()
        .map_or(i_slint_core::items::CrossAxisAlignment::default(), |nr| {
            eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
        });
    let flex_wrap = flexbox_layout
        .flex_wrap
        .as_ref()
        .map_or(i_slint_core::items::FlexboxLayoutWrap::default(), |nr| {
            eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
        });

    let (padding_h, spacing_h) =
        padding_and_spacing(&flexbox_layout.geometry, Orientation::Horizontal, &expr_eval);
    let (padding_v, spacing_v) =
        padding_and_spacing(&flexbox_layout.geometry, Orientation::Vertical, &expr_eval);

    let data = core_layout::FlexboxLayoutData {
        width: width_ref.as_ref().map(&expr_eval).unwrap_or(0.),
        height: height_ref.as_ref().map(&expr_eval).unwrap_or(0.),
        spacing_h,
        spacing_v,
        padding_h,
        padding_v,
        alignment,
        direction,
        align_content,
        cross_axis_alignment,
        flex_wrap,
        cells_h: Slice::from(cells_h.as_slice()),
        cells_v: Slice::from(cells_v.as_slice()),
    };
    let ri = Slice::from(repeated_indices.as_slice());

    let window_adapter = component.window_adapter();

    // Build measure callback that computes constrained layout_info for items
    // that support height-for-width (Text with wrap, Image with aspect ratio,
    // and component instances with a synthesized
    // `layoutinfo-{v,h}-with-constraint`).
    //
    // Collect `(element, has_constrained_layoutinfo_{v,h})` so we can
    // dispatch component instances through the parametrized layout-info
    // function rather than through the Item vtable (which returns trivial
    // info for the Empty wrapper of a sub-component instance).
    struct ChildElem {
        elem: ElementRc,
        has_constrained_layoutinfo_v: bool,
        has_constrained_layoutinfo_h: bool,
        /// True when the cell aggregates layoutinfo from its own subtree
        /// (set by `default_geometry::gen_layout_info_prop`) — typical for
        /// component-wrappers like a Rectangle containing a layout. In
        /// that case the Item vtable's `layout_info` on the wrapper item
        /// returns trivial info that doesn't reflect the aggregated
        /// constraints; we read the aggregated property directly.
        has_aggregated_info: bool,
        /// For a repeated cell, the instance to query — its constrained
        /// layout-info function lives in the instance's own item tree, not in
        /// the parent `component`. `None` for a static cell.
        repeated_instance: Option<crate::dynamic_item_tree::DynamicComponentVRc>,
    }
    let mut child_elems: Vec<Option<ChildElem>> = Vec::new();
    for layout_elem in &flexbox_layout.elems {
        let placeholder = layout_elem.item.element.borrow();
        let repeated = placeholder.repeated.is_some();
        // For a repeated cell, query against the repeated component's root
        // element (where its `layoutinfo-*-with-constraint` is reachable) in the
        // instance's own item tree; for a static cell, against the cell element
        // in the parent `component`.
        let query_elem = if repeated {
            placeholder.base_type.as_component().root_element.clone()
        } else {
            layout_elem.item.element.clone()
        };
        drop(placeholder);
        let qe = query_elem.borrow();
        let has_constrained_layoutinfo_v = qe.inherited_layout_info_v_with_constraint().is_some();
        let has_constrained_layoutinfo_h = qe.inherited_layout_info_h_with_constraint().is_some();
        let has_aggregated_info = qe.layout_info_prop.is_some();
        drop(qe);
        if repeated {
            // One entry per instance: each is re-measured through its own item
            // tree at the assigned cross size.
            let component_vec = repeater_instances(component, &layout_elem.item.element);
            for instance in component_vec {
                child_elems.push(Some(ChildElem {
                    elem: query_elem.clone(),
                    has_constrained_layoutinfo_v,
                    has_constrained_layoutinfo_h,
                    has_aggregated_info,
                    repeated_instance: Some(instance),
                }));
            }
        } else {
            child_elems.push(Some(ChildElem {
                elem: query_elem,
                has_constrained_layoutinfo_v,
                has_constrained_layoutinfo_h,
                has_aggregated_info,
                repeated_instance: None,
            }));
        }
    }

    let mut measure = |child_index: usize,
                       known_w: Option<f32>,
                       known_h: Option<f32>|
     -> (f32, f32) {
        let default_w = cells_h.get(child_index).map_or(0., |c| c.constraint.preferred_bounded());
        let default_h = cells_v.get(child_index).map_or(0., |c| c.constraint.preferred_bounded());
        let w = known_w.unwrap_or(default_w);
        let h = known_h.unwrap_or(default_h);

        let ce = match child_elems.get(child_index) {
            Some(Some(c)) => c,
            _ => return (w, h),
        };

        // Cells whose layoutinfo aggregates from a sub-tree (set by
        // default_geometry) or that have a parametrized layout-info
        // function need to be queried by NamedReference. The Item
        // vtable's `layout_info` on the wrapper item returns trivial
        // info that ignores the aggregated children.
        let use_property_lookup = ce.has_aggregated_info
            || ce.has_constrained_layoutinfo_v
            || ce.has_constrained_layoutinfo_h;

        // Query the cell's constrained layout-info. A repeated cell lives in
        // its own instance's item tree (where its `layoutinfo-*-with-constraint`
        // function is), so re-measure it there at the assigned cross size; a
        // static cell is queried through the parent `component`.
        let query = |orientation, constraint: Option<f32>| -> core_layout::LayoutInfo {
            match &ce.repeated_instance {
                Some(instance) => {
                    generativity::make_guard!(guard);
                    let unerased = instance.unerase(guard);
                    get_layout_info_with_constraint(
                        &ce.elem,
                        unerased.borrow_instance(),
                        &window_adapter,
                        orientation,
                        constraint,
                    )
                }
                None => get_layout_info_with_constraint(
                    &ce.elem,
                    component,
                    &window_adapter,
                    orientation,
                    constraint,
                ),
            }
        };

        if known_w.is_some() && known_h.is_none() {
            if use_property_lookup {
                let v_info =
                    query(Orientation::Vertical, ce.has_constrained_layoutinfo_v.then_some(w));
                return (w, v_info.preferred_bounded());
            }
            // Builtin path (Text, Image): use the Item vtable's layout_info,
            // which honors the cross-axis constraint. This resolves the item in
            // the parent `component`'s item tree, so it does not apply to a
            // repeated cell (which lives in its own instance): a
            // height-for-width repeated cell takes the `query` path above, and a
            // non-height-for-width one keeps its width-independent default `h`.
            if let Some(item_within) = ce
                .repeated_instance
                .is_none()
                .then(|| component.description.items.get(ce.elem.borrow().id.as_str()))
                .flatten()
            {
                let item_comp = component.self_weak().get().unwrap().upgrade().unwrap();
                let item_rc =
                    ItemRc::new(vtable::VRc::into_dyn(item_comp), item_within.item_index());
                let item = unsafe { item_within.item_from_item_tree(component.as_ptr()) };
                let v_info = item.as_ref().layout_info(
                    to_runtime(Orientation::Vertical),
                    w,
                    &window_adapter,
                    &item_rc,
                );
                return (w, v_info.preferred_bounded());
            }
            return (w, h);
        }
        if known_h.is_some() && known_w.is_none() {
            if use_property_lookup {
                let h_info =
                    query(Orientation::Horizontal, ce.has_constrained_layoutinfo_h.then_some(h));
                return (h_info.preferred_bounded(), h);
            }
            // Builtin path, symmetric to the known-w branch above: parent-tree
            // lookup only, so it is skipped for a repeated cell.
            if let Some(item_within) = ce
                .repeated_instance
                .is_none()
                .then(|| component.description.items.get(ce.elem.borrow().id.as_str()))
                .flatten()
            {
                let item_comp = component.self_weak().get().unwrap().upgrade().unwrap();
                let item_rc =
                    ItemRc::new(vtable::VRc::into_dyn(item_comp), item_within.item_index());
                let item = unsafe { item_within.item_from_item_tree(component.as_ptr()) };
                let h_info = item.as_ref().layout_info(
                    to_runtime(Orientation::Horizontal),
                    h,
                    &window_adapter,
                    &item_rc,
                );
                return (h_info.preferred_bounded(), h);
            }
            return (w, h);
        }
        (w, h)
    };

    core_layout::solve_flexbox_layout_with_measure(&data, ri, Some(&mut measure)).into()
}

fn flexbox_layout_direction(
    flexbox_layout: &i_slint_compiler::layout::FlexboxLayout,
    local_context: &EvalLocalContext,
) -> FlexboxLayoutDirection {
    flexbox_layout
        .direction
        .as_ref()
        .and_then(|nr| {
            let value =
                eval::load_property(local_context.component_instance, &nr.element(), nr.name())
                    .ok()?;
            if let Value::EnumerationValue(_, variant) = &value {
                match variant.as_str() {
                    "row" => Some(FlexboxLayoutDirection::Row),
                    "row-reverse" => Some(FlexboxLayoutDirection::RowReverse),
                    "column" => Some(FlexboxLayoutDirection::Column),
                    "column-reverse" => Some(FlexboxLayoutDirection::ColumnReverse),
                    _ => None,
                }
            } else {
                None
            }
        })
        .unwrap_or(FlexboxLayoutDirection::Row)
}

pub(crate) fn compute_flexbox_layout_info(
    flexbox_layout: &i_slint_compiler::layout::FlexboxLayout,
    orientation: Orientation,
    local_context: &mut EvalLocalContext,
    cross_axis_size: Option<f32>,
) -> Value {
    let component = local_context.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };

    // `cross_axis_size` carries a width when called from a
    // `layoutinfo-v-with-constraint` body, a height from a
    // `layoutinfo-h-with-constraint` body. Route it to the matching
    // cell-list so cells don't receive a dimension on the wrong axis.
    let (width_override, height_override) = match orientation {
        Orientation::Vertical => (cross_axis_size, None),
        Orientation::Horizontal => (None, cross_axis_size),
    };
    // Subtract padding so height-for-width cells are measured at the content
    // width they are actually laid out at, not the padded outer width.
    let width_override = width_override.map(|w| {
        let (pad_h, _) =
            padding_and_spacing(&flexbox_layout.geometry, Orientation::Horizontal, &expr_eval);
        w - pad_h.begin - pad_h.end
    });
    let height_override = height_override.map(|h| {
        let (pad_v, _) =
            padding_and_spacing(&flexbox_layout.geometry, Orientation::Vertical, &expr_eval);
        h - pad_v.begin - pad_v.end
    });
    let (cells_h, cells_v, _repeated_indices) = flexbox_layout_data(
        flexbox_layout,
        component,
        &expr_eval,
        local_context,
        width_override,
        height_override,
    );

    // Get the direction from the property binding
    let direction = flexbox_layout_direction(flexbox_layout, local_context);

    // Determine if we're on the main axis or cross axis
    let is_main_axis = matches!(
        (direction, orientation),
        (FlexboxLayoutDirection::Row | FlexboxLayoutDirection::RowReverse, Orientation::Horizontal)
            | (
                FlexboxLayoutDirection::Column | FlexboxLayoutDirection::ColumnReverse,
                Orientation::Vertical
            )
    );

    let (padding_h, spacing_h) =
        padding_and_spacing(&flexbox_layout.geometry, Orientation::Horizontal, &expr_eval);
    let (padding_v, spacing_v) =
        padding_and_spacing(&flexbox_layout.geometry, Orientation::Vertical, &expr_eval);

    let flex_wrap = flexbox_layout
        .flex_wrap
        .as_ref()
        .map_or(i_slint_core::items::FlexboxLayoutWrap::default(), |nr| {
            eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
        });

    if is_main_axis {
        let (cells, spacing, padding) = match orientation {
            Orientation::Horizontal => (&cells_h, spacing_h, &padding_h),
            Orientation::Vertical => (&cells_v, spacing_v, &padding_v),
        };
        core_layout::flexbox_layout_info_main_axis(
            Slice::from(cells.as_slice()),
            spacing,
            padding,
            flex_wrap,
        )
        .into()
    } else {
        // Override when set (e.g., from a `layoutinfo-h-with-constraint`
        // body); otherwise self's perpendicular dimension. The override
        // path breaks the cycle for nested perpendicular flexboxes.
        let constraint_size = cross_axis_size.unwrap_or_else(|| match orientation {
            Orientation::Horizontal => {
                let height_ref = &flexbox_layout.geometry.rect.height_reference;
                height_ref.as_ref().map(&expr_eval).unwrap_or(0.)
            }
            Orientation::Vertical => {
                let width_ref = &flexbox_layout.geometry.rect.width_reference;
                width_ref.as_ref().map(&expr_eval).unwrap_or(0.)
            }
        });
        core_layout::flexbox_layout_info_cross_axis(
            Slice::from(cells_h.as_slice()),
            Slice::from(cells_v.as_slice()),
            spacing_h,
            spacing_v,
            &padding_h,
            &padding_v,
            direction,
            flex_wrap,
            constraint_size,
        )
        .into()
    }
}

fn flexbox_layout_data(
    flexbox_layout: &i_slint_compiler::layout::FlexboxLayout,
    component: InstanceRef,
    expr_eval: &impl Fn(&NamedReference) -> f32,
    _local_context: &mut EvalLocalContext,
    width_override: Option<f32>,
    height_override: Option<f32>,
) -> (Vec<core_layout::FlexboxLayoutItemInfo>, Vec<core_layout::FlexboxLayoutItemInfo>, Vec<u32>) {
    let window_adapter = component.window_adapter();
    let mut cells_h = Vec::with_capacity(flexbox_layout.elems.len());
    let mut cells_v = Vec::with_capacity(flexbox_layout.elems.len());
    let mut repeated_indices = Vec::new();

    // First pass: collect horizontal layout_info for all children (no cycle risk)
    // and flex properties. Store element refs for the second pass.
    struct ChildInfo {
        flex_grow: f32,
        flex_shrink: f32,
        flex_basis: f32,
        flex_align_self: i_slint_core::items::FlexboxLayoutAlignSelf,
        flex_order: i32,
    }
    let mut static_children: Vec<Option<ChildInfo>> = Vec::new(); // None = repeater
    // Instances of each repeater, in `elems` order, so the second pass doesn't walk them again.
    let mut repeater_instance_vecs: Vec<Vec<crate::dynamic_item_tree::DynamicComponentVRc>> =
        Vec::new();

    for layout_elem in &flexbox_layout.elems {
        if layout_elem.item.element.borrow().repeated.is_some() {
            let component_vec = repeater_instances(component, &layout_elem.item.element);
            repeated_indices.push(cells_h.len() as u32);
            repeated_indices.push(component_vec.len() as u32);
            cells_h.extend(component_vec.iter().map(|x| {
                x.as_pin_ref().flexbox_layout_item_info(to_runtime(Orientation::Horizontal), None)
            }));
            cells_v.extend(component_vec.iter().map(|x| {
                x.as_pin_ref().flexbox_layout_item_info(to_runtime(Orientation::Vertical), None)
            }));
            static_children.resize_with(static_children.len() + component_vec.len(), || None);
            repeater_instance_vecs.push(component_vec);
        } else {
            // Dispatch via `layoutinfo-h-with-constraint` for cells that
            // have one, avoiding the `self.height` read that would cycle.
            // Use the height-override when set, else `f32::MAX` so the
            // runtime treats it as "no wrap needed" — gives the natural
            // max-cell-width result rather than the heuristic.
            let h_constraint = layout_elem
                .item
                .element
                .borrow()
                .inherited_layout_info_h_with_constraint()
                .is_some()
                .then(|| height_override.unwrap_or(f32::MAX));
            let mut layout_info_h = get_layout_info_with_constraint(
                &layout_elem.item.element,
                component,
                &window_adapter,
                Orientation::Horizontal,
                h_constraint,
            );
            fill_layout_info_constraints(
                &mut layout_info_h,
                &layout_elem.item.constraints,
                Orientation::Horizontal,
                expr_eval,
            );
            // Don't collect cells_v in the first pass — it may trigger a circular
            // dependency for height-for-width items (Text with wrap, Image).
            // The second pass fills in cells_v with the width constraint.
            let flex_grow = layout_elem.flex_grow.as_ref().map(&expr_eval).unwrap_or(0.0);
            let flex_shrink = layout_elem.flex_shrink.as_ref().map(&expr_eval).unwrap_or(1.0);
            let flex_basis = layout_elem.flex_basis.as_ref().map(&expr_eval).unwrap_or(-1.0);
            let align_self = layout_elem
                .align_self
                .as_ref()
                .map(|nr| {
                    eval::load_property(component, &nr.element(), nr.name())
                        .unwrap()
                        .try_into()
                        .unwrap()
                })
                .unwrap_or(i_slint_core::items::FlexboxLayoutAlignSelf::default());
            let order = layout_elem.order.as_ref().map(expr_eval).unwrap_or(0.0) as i32;
            cells_h.push(core_layout::FlexboxLayoutItemInfo {
                constraint: layout_info_h,
                flex_grow,
                flex_shrink,
                flex_basis,
                flex_align_self: align_self,
                flex_order: order,
            });
            // Placeholder for cells_v — filled in second pass
            cells_v.push(core_layout::FlexboxLayoutItemInfo::default());
            static_children.push(Some(ChildInfo {
                flex_grow,
                flex_shrink,
                flex_basis,
                flex_align_self: align_self,
                flex_order: order,
            }));
        }
    }

    // Second pass: collect vertical layout_info with a width constraint.
    // For column direction, use the container width (items get stretched to it).
    // Otherwise use the item's horizontal preferred size.
    let mut cell_idx = 0usize;
    let mut repeater_idx = 0usize;
    for layout_elem in &flexbox_layout.elems {
        if layout_elem.item.element.borrow().repeated.is_some() {
            // Re-measure each height-for-width repeated instance at the width
            // constraint (container width for a column flex), mirroring the
            // static branch below — the first pass filled cells_v at the
            // instance's preferred width (single line). Keep the flex props set
            // in the first pass; only overwrite the vertical constraint.
            let rep_root =
                layout_elem.item.element.borrow().base_type.as_component().root_element.clone();
            let is_height_for_width =
                rep_root.borrow().inherited_layout_info_v_with_constraint().is_some();
            let component_vec = &repeater_instance_vecs[repeater_idx];
            repeater_idx += 1;
            for instance in component_vec {
                if is_height_for_width {
                    let width_constraint = width_override
                        .unwrap_or_else(|| cells_h[cell_idx].constraint.preferred_bounded());
                    generativity::make_guard!(guard);
                    let unerased = instance.unerase(guard);
                    let instance_ref = unerased.borrow_instance();
                    let mut layout_info_v = get_layout_info_with_constraint(
                        &rep_root,
                        instance_ref,
                        &window_adapter,
                        Orientation::Vertical,
                        Some(width_constraint),
                    );
                    // The constraints' NamedReferences point to elements inside
                    // the repeated sub-component, so evaluate them in that
                    // instance's context, not the outer component's.
                    let instance_expr_eval = |nr: &NamedReference| -> f32 {
                        eval::load_property(instance_ref, &nr.element(), nr.name())
                            .unwrap()
                            .try_into()
                            .unwrap()
                    };
                    fill_layout_info_constraints(
                        &mut layout_info_v,
                        &layout_elem.item.constraints,
                        Orientation::Vertical,
                        &instance_expr_eval,
                    );
                    cells_v[cell_idx].constraint = layout_info_v;
                }
                cell_idx += 1;
            }
        } else {
            let width_constraint =
                width_override.unwrap_or_else(|| cells_h[cell_idx].constraint.preferred_bounded());
            let mut layout_info_v = get_layout_info_with_constraint(
                &layout_elem.item.element,
                component,
                &window_adapter,
                Orientation::Vertical,
                Some(width_constraint),
            );
            fill_layout_info_constraints(
                &mut layout_info_v,
                &layout_elem.item.constraints,
                Orientation::Vertical,
                expr_eval,
            );
            if let Some(info) = &static_children[cell_idx] {
                cells_v[cell_idx] = core_layout::FlexboxLayoutItemInfo {
                    constraint: layout_info_v,
                    flex_grow: info.flex_grow,
                    flex_shrink: info.flex_shrink,
                    flex_basis: info.flex_basis,
                    flex_align_self: info.flex_align_self,
                    flex_order: info.flex_order,
                };
            }
            cell_idx += 1;
        }
    }

    (cells_h, cells_v, repeated_indices)
}

/// Determine the evaluated padding and spacing values from the layout geometry
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
    rep.0.as_ref().track_instance_changes();
    rep.0.as_ref().instances_vec()
}

fn grid_layout_input_data(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    ctx: &EvalLocalContext,
    repeater_steps: &[u32],
) -> Vec<GridLayoutInputData> {
    let component = ctx.component_instance;
    let mut result = Vec::with_capacity(grid_layout.elems.len());
    let mut after_repeater_in_same_row = false;
    let mut new_row = true;
    let mut repeater_idx = 0usize;
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

                if let Some(children) = elem.cell.borrow().child_items.as_ref() {
                    // Repeated row
                    new_row = true;
                    let start_count = result.len();

                    // Single pass in declaration order: push statics and inner-repeater
                    // auto-cells interleaved so that column assignments match template order.
                    // (A two-pass approach that appended all inner-repeater cells after all
                    // statics would produce wrong column assignments, and only tracking the
                    // last Repeated entry would miss earlier conditionals/for-loops.)
                    for child_template in children {
                        match child_template {
                            i_slint_compiler::layout::RowChildTemplate::Static(child_item) => {
                                let (row_val, col_val, rowspan_val, colspan_val) = {
                                    let element_ref = child_item.element.borrow();
                                    let child_cell =
                                        element_ref.grid_layout_cell.as_ref().unwrap().borrow();
                                    (
                                        eval_or_default(&child_cell.row_expr, sub_instance_ref),
                                        eval_or_default(&child_cell.col_expr, sub_instance_ref),
                                        eval_or_default(&child_cell.rowspan_expr, sub_instance_ref),
                                        eval_or_default(&child_cell.colspan_expr, sub_instance_ref),
                                    )
                                };
                                result.push(GridLayoutInputData {
                                    new_row,
                                    col: col_val,
                                    row: row_val,
                                    colspan: colspan_val,
                                    rowspan: rowspan_val,
                                });
                                new_row = false;
                            }
                            i_slint_compiler::layout::RowChildTemplate::Repeated {
                                repeated_element,
                                ..
                            } => {
                                // colspan/rowspan live on the inner sub-component's root,
                                // evaluated per inner instance.
                                let inner_root = repeated_element
                                    .borrow()
                                    .base_type
                                    .as_component()
                                    .root_element
                                    .clone();
                                let (rowspan_expr, colspan_expr) = {
                                    let element_ref = inner_root.borrow();
                                    let child_cell =
                                        element_ref.grid_layout_cell.as_ref().unwrap().borrow();
                                    (
                                        child_cell.rowspan_expr.clone(),
                                        child_cell.colspan_expr.clone(),
                                    )
                                };
                                let inner_instances =
                                    repeater_instances(sub_instance_ref, repeated_element);
                                for (i, erased_inner) in inner_instances.iter().enumerate() {
                                    generativity::make_guard!(inner_guard);
                                    let inner_comp = erased_inner.as_pin_ref();
                                    let inner_instance_ref = unsafe {
                                        InstanceRef::from_pin_ref(inner_comp.borrow(), inner_guard)
                                    };
                                    result.push(GridLayoutInputData {
                                        new_row: i == 0 && new_row,
                                        rowspan: eval_or_default(&rowspan_expr, inner_instance_ref),
                                        colspan: eval_or_default(&colspan_expr, inner_instance_ref),
                                        ..Default::default()
                                    });
                                }
                                if !inner_instances.is_empty() {
                                    new_row = false;
                                }
                            }
                        }
                    }
                    // Pad to match max step count for this repeater (handles jagged arrays)
                    let cells_pushed = result.len() - start_count;
                    let expected_step =
                        repeater_steps.get(repeater_idx).copied().unwrap_or(0) as usize;
                    for _ in cells_pushed..expected_step {
                        result.push(GridLayoutInputData::default());
                    }
                } else {
                    // Single repeated item
                    let cell = elem.cell.borrow();
                    let row = eval_or_default(&cell.row_expr, sub_instance_ref);
                    let col = eval_or_default(&cell.col_expr, sub_instance_ref);
                    let rowspan = eval_or_default(&cell.rowspan_expr, sub_instance_ref);
                    let colspan = eval_or_default(&cell.colspan_expr, sub_instance_ref);
                    result.push(GridLayoutInputData { new_row, col, row, colspan, rowspan });
                    new_row = false;
                }
            }
            repeater_idx += 1;
            after_repeater_in_same_row = true;
        } else {
            let new_row =
                if cell_new_row || !after_repeater_in_same_row { cell_new_row } else { new_row };
            let row = eval_or_default(&elem.cell.borrow().row_expr, component);
            let col = eval_or_default(&elem.cell.borrow().col_expr, component);
            let rowspan = eval_or_default(&elem.cell.borrow().rowspan_expr, component);
            let colspan = eval_or_default(&elem.cell.borrow().colspan_expr, component);
            result.push(GridLayoutInputData { new_row, col, row, colspan, rowspan });
        }
    }
    result
}

/// Count the actual runtime children for a repeated row.
/// For rows without inner repeaters, this is just the child_items count.
/// For rows with inner repeaters, the Repeated template expands to actual inner instances.
fn row_runtime_child_count(
    child_items: &[i_slint_compiler::layout::RowChildTemplate],
    sub_instance_ref: InstanceRef,
) -> usize {
    let mut count = 0;
    for child in child_items {
        if let Some(repeated_element) = child.repeated_element() {
            count += repeater_instances(sub_instance_ref, repeated_element).len();
        } else {
            count += 1;
        }
    }
    count
}

fn grid_repeater_indices(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    ctx: &mut EvalLocalContext,
    repeater_steps: &[u32],
) -> Vec<u32> {
    let component = ctx.component_instance;
    let mut repeater_indices = Vec::new();
    let mut num_cells = 0;
    let mut step_idx = 0;
    for elem in grid_layout.elems.iter() {
        if elem.item.element.borrow().repeated.is_some() {
            let component_vec = repeater_instances(component, &elem.item.element);
            repeater_indices.push(num_cells as _);
            repeater_indices.push(component_vec.len() as _);
            let item_count = repeater_steps[step_idx] as usize;
            num_cells += component_vec.len() * item_count;
            step_idx += 1;
        } else {
            num_cells += 1;
        }
    }
    repeater_indices
}

fn grid_repeater_steps(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    ctx: &mut EvalLocalContext,
) -> Vec<u32> {
    let component = ctx.component_instance;
    let mut repeater_steps = Vec::new();
    for elem in grid_layout.elems.iter() {
        if elem.item.element.borrow().repeated.is_some() {
            let item_count = match &elem.cell.borrow().child_items {
                Some(ci)
                    if ci.iter().any(i_slint_compiler::layout::RowChildTemplate::is_repeated) =>
                {
                    // Compute max runtime count across all instances (padding with empty cells didn't happen yet)
                    let component_vec = repeater_instances(component, &elem.item.element);
                    component_vec
                        .iter()
                        .map(|sub| {
                            generativity::make_guard!(guard);
                            let sub_pin = sub.as_pin_ref();
                            let sub_ref =
                                unsafe { InstanceRef::from_pin_ref(sub_pin.borrow(), guard) };
                            row_runtime_child_count(ci, sub_ref)
                        })
                        .max()
                        .unwrap_or(0)
                }
                Some(ci) => ci.len(),
                None => 1,
            };
            repeater_steps.push(item_count as u32);
        }
    }
    repeater_steps
}

fn grid_layout_constraints(
    grid_layout: &i_slint_compiler::layout::GridLayout,
    orientation: Orientation,
    ctx: &mut EvalLocalContext,
    repeater_steps: &[u32],
    cross_axis_size: Option<f32>,
) -> Vec<core_layout::LayoutItemInfo> {
    let component = ctx.component_instance;
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };
    let mut constraints = Vec::with_capacity(grid_layout.elems.len());

    let mut repeater_idx = 0usize;
    for layout_elem in grid_layout.elems.iter() {
        if layout_elem.item.element.borrow().repeated.is_some() {
            let component_vec = repeater_instances(component, &layout_elem.item.element);
            let child_items = layout_elem.cell.borrow().child_items.clone();
            let has_children = child_items.is_some();
            if has_children {
                // Repeated row
                let ci = child_items.as_ref().unwrap();
                let step = repeater_steps.get(repeater_idx).copied().unwrap_or(0) as usize;
                for sub_comp in &component_vec {
                    let per_instance_start = constraints.len();
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

                    // Iterate over the child templates: static children get their layout info
                    // from the Row sub-component; nested repeater children get theirs from the
                    // inner repeater instances.
                    for child_template in ci.iter() {
                        match child_template {
                            i_slint_compiler::layout::RowChildTemplate::Static(child_item) => {
                                let mut layout_info = crate::eval_layout::get_layout_info(
                                    &child_item.element,
                                    sub_instance_ref,
                                    &sub_instance_ref.window_adapter(),
                                    orientation,
                                );
                                fill_layout_info_constraints(
                                    &mut layout_info,
                                    &child_item.constraints,
                                    orientation,
                                    &expr_eval,
                                );
                                constraints
                                    .push(core_layout::LayoutItemInfo { constraint: layout_info });
                            }
                            i_slint_compiler::layout::RowChildTemplate::Repeated {
                                item: child_item,
                                repeated_element,
                            } => {
                                // Get the inner repeater instances from within this Row instance
                                let inner_instances =
                                    repeater_instances(sub_instance_ref, repeated_element);
                                for inner_comp in &inner_instances {
                                    let inner_pin = inner_comp.as_pin_ref();
                                    let mut layout_info =
                                        inner_pin.layout_item_info(to_runtime(orientation), None);
                                    // Constraints' NamedReferences point to elements inside the
                                    // inner repeated component, so evaluate in that context.
                                    generativity::make_guard!(inner_guard);
                                    let inner_borrow = inner_pin.borrow();
                                    let inner_instance_ref = unsafe {
                                        InstanceRef::from_pin_ref(inner_borrow, inner_guard)
                                    };
                                    let inner_expr_eval = |nr: &NamedReference| -> f32 {
                                        eval::load_property(
                                            inner_instance_ref,
                                            &nr.element(),
                                            nr.name(),
                                        )
                                        .unwrap()
                                        .try_into()
                                        .unwrap()
                                    };
                                    fill_layout_info_constraints(
                                        &mut layout_info.constraint,
                                        &child_item.constraints,
                                        orientation,
                                        &inner_expr_eval,
                                    );
                                    constraints.push(layout_info);
                                }
                            }
                        }
                    }
                    // Pad this instance to the step size (handles jagged arrays where
                    // inner repeaters have different lengths across outer Row instances).
                    let pushed = constraints.len() - per_instance_start;
                    for _ in pushed..step {
                        constraints.push(core_layout::LayoutItemInfo::default());
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
            repeater_idx += 1;
        } else {
            let cross_axis =
                cross_axis_size_for_cell(&layout_elem.item.element, orientation, cross_axis_size);
            let mut layout_info = get_layout_info_with_constraint(
                &layout_elem.item.element,
                component,
                &component.window_adapter(),
                orientation,
                cross_axis,
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

/// Collect all elements in this layout and store the LayoutItemInfo of it for further calculation
fn box_layout_data(
    box_layout: &i_slint_compiler::layout::BoxLayout,
    orientation: Orientation,
    component: InstanceRef,
    expr_eval: &impl Fn(&NamedReference) -> f32,
    mut repeater_indices: Option<&mut Vec<u32>>,
    cross_axis_size: Option<f32>,
    local_context: &mut EvalLocalContext,
    available_cross: Option<f32>,
) -> (Vec<core_layout::LayoutItemInfo>, i_slint_core::items::LayoutAlignment) {
    let window_adapter = component.window_adapter();
    let mut cells = Vec::with_capacity(box_layout.elems.len());
    for cell in &box_layout.elems {
        if cell.element.borrow().repeated.is_some() {
            // Collect all repeated elements
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
            // Collect non repeated elements
            let cross_axis = cross_axis_size_for_cell(&cell.element, orientation, cross_axis_size);
            let mut layout_info = get_layout_info_with_constraint(
                &cell.element,
                component,
                &window_adapter,
                orientation,
                cross_axis,
            );
            clamp_wrapping_flex_cross_preferred(
                &mut layout_info,
                &cell.element,
                box_layout,
                orientation,
                component,
                &expr_eval,
                local_context,
                available_cross,
            );
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

/// When a wrapping FlexboxLayout is a cell of a perpendicular box layout (its
/// main axis is the parent's cross axis), a non-stretch parent gives the cell
/// its `preferred` cross size. The flex's plain preferred is the compact
/// sqrt-area "square" (it wraps), but with room it should fill a single line
/// and only wrap when the available cross size can't hold it. Clamp the
/// preferred to `min(available, unwrapped)`, so the height it is given equals
/// the height its width was computed at: no wrap when tall, no overlap with
/// siblings. Only at solve time (`available_cross` is `Some`); during
/// layout-info aggregation the sqrt preferred is kept so the flex can still
/// size a window.
#[allow(clippy::too_many_arguments)]
fn clamp_wrapping_flex_cross_preferred(
    layout_info: &mut core_layout::LayoutInfo,
    elem: &ElementRc,
    box_layout: &i_slint_compiler::layout::BoxLayout,
    orientation: Orientation,
    component: InstanceRef,
    expr_eval: &impl Fn(&NamedReference) -> f32,
    local_context: &mut EvalLocalContext,
    available_cross: Option<f32>,
) {
    let Some(available) = available_cross else { return };
    if orientation == box_layout.orientation {
        return;
    }
    let Some(fl) = i_slint_compiler::layout::FlexboxLayout::from_element(elem) else { return };

    // The flex's main axis must be the parent's cross axis, and it must wrap.
    let direction = flexbox_layout_direction(&fl, local_context);
    let main_is_cross = matches!(
        (direction, orientation),
        (FlexboxLayoutDirection::Row | FlexboxLayoutDirection::RowReverse, Orientation::Horizontal)
            | (
                FlexboxLayoutDirection::Column | FlexboxLayoutDirection::ColumnReverse,
                Orientation::Vertical
            )
    );
    if !main_is_cross {
        return;
    }
    let flex_wrap =
        fl.flex_wrap.as_ref().map_or(i_slint_core::items::FlexboxLayoutWrap::default(), |nr| {
            eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
        });
    if matches!(flex_wrap, i_slint_core::items::FlexboxLayoutWrap::NoWrap) {
        return;
    }

    let (cells_h, cells_v, _ri) =
        flexbox_layout_data(&fl, component, expr_eval, local_context, None, None);
    let (cells, padding, spacing) = match orientation {
        Orientation::Horizontal => {
            let (padding, spacing) =
                padding_and_spacing(&fl.geometry, Orientation::Horizontal, expr_eval);
            (cells_h, padding, spacing)
        }
        Orientation::Vertical => {
            let (padding, spacing) =
                padding_and_spacing(&fl.geometry, Orientation::Vertical, expr_eval);
            (cells_v, padding, spacing)
        }
    };
    let unwrapped = core_layout::flexbox_layout_unwrapped_main(
        Slice::from(cells.as_slice()),
        spacing,
        &padding,
    );
    layout_info.preferred = available.min(unwrapped);
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
    get_layout_info_with_constraint(elem, component, window_adapter, orientation, None)
}

pub(crate) fn get_layout_info_with_constraint(
    elem: &ElementRc,
    component: InstanceRef,
    window_adapter: &Rc<dyn WindowAdapter>,
    orientation: Orientation,
    cross_axis_constraint: Option<f32>,
) -> core_layout::LayoutInfo {
    // With a constraint and a parameterized layout-info function on the
    // cell, call it instead of reading the cell's perpendicular property.
    // Use the inherited lookup so component-instance cells pick up a
    // `layoutinfo-{v,h}-with-constraint` declared on the base component's
    // root_element (the cell itself doesn't carry the binding).
    let parameterized_nr = if cross_axis_constraint.is_some() {
        match orientation {
            Orientation::Vertical => elem.borrow().inherited_layout_info_v_with_constraint(),
            Orientation::Horizontal => elem.borrow().inherited_layout_info_h_with_constraint(),
        }
    } else {
        None
    };
    if let Some(nr) = parameterized_nr {
        let arg = cross_axis_constraint.unwrap();
        let v = eval::call_function(
            &eval::ComponentInstance::InstanceRef(component),
            &nr.element(),
            nr.name(),
            vec![Value::Number(arg as f64)],
        )
        .expect("layoutinfo-{h,v}-with-constraint is a synthesized pure function");
        return v.try_into().unwrap();
    }

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
                cross_axis_constraint.unwrap_or(-1.),
                window_adapter,
                &ItemRc::new(vtable::VRc::into_dyn(item_comp), item.item_index()),
            )
        }
    }
}

/// Decide the cross-axis constraint to forward to a cell's perpendicular
/// layout-info call. Returns `Some` only when the parent supplied the cross
/// dimension and the cell consumes it: a height-for-width cell on the vertical
/// pass (wrapped Text/Image, or a `layoutinfo-v-with-constraint` component), or a
/// width-for-height cell on the horizontal pass (a `layoutinfo-h-with-constraint`
/// component, e.g. a column FlexboxLayout).
fn cross_axis_size_for_cell(
    elem: &ElementRc,
    orientation: Orientation,
    parent_cross_axis_size: Option<f32>,
) -> Option<f32> {
    let cross = parent_cross_axis_size?;
    let elem_b = elem.borrow();
    if orientation == Orientation::Horizontal {
        // Width-for-height cells (e.g. a column FlexboxLayout) carry a synthesized
        // layoutinfo-h-with-constraint; forward the cross size so they compute their
        // width from it instead of reading self.height (which would cycle through
        // the parent's vertical pass).
        return elem_b.inherited_layout_info_h_with_constraint().is_some().then_some(cross);
    }
    if elem_b.layout_info_v_with_constraint.is_some() {
        return Some(cross);
    }
    // For builtin height-for-width items, the existing VTable cross_axis_constraint
    // parameter mechanism is what consumes the value; conservatively
    // forward it for any element without its own layoutinfo-v property
    // (i.e. anything that ends up calling the builtin VTable).
    if elem_b.layout_info_prop(Orientation::Vertical).is_none() {
        return Some(cross);
    }
    None
}
