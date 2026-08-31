// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::BTreeMap;
use std::sync::Arc;

use itertools::Either;
use smol_str::SmolStr;

use super::lower_to_item_tree::LoweredElement;
use super::{GridLayoutRepeatedElement, LayoutRepeatedElement};
use crate::expression_tree::MinMaxOp;
use crate::langtype::{BuiltinStruct, EnumerationValue, Struct, Type};
use crate::layout::{FlexboxAxisRelation, GridLayoutCell, Orientation, RowColExpr};
use crate::llr::ArrayOutput as llr_ArrayOutput;
use crate::llr::Expression as llr_Expression;
use crate::llr::{BoxMeasureCell, FlexboxMeasureCell, FlexboxMeasureCellKind};
use crate::namedreference::NamedReference;
use crate::object_tree::ElementRc;

use super::lower_expression::{ExpressionLoweringCtx, make_struct};

fn empty_int32_slice() -> llr_Expression {
    llr_Expression::Array {
        element_ty: Type::Int32,
        values: Vec::new(),
        output: llr_ArrayOutput::Slice,
    }
}

pub(super) fn compute_grid_layout_info(
    layout_organized_data_prop: &NamedReference,
    layout: &crate::layout::GridLayout,
    o: Orientation,
    ctx: &mut ExpressionLoweringCtx,
    cross_axis_size_override: Option<&crate::expression_tree::Expression>,
) -> llr_Expression {
    let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
    let organized_cells = ctx.map_property_reference(layout_organized_data_prop);
    let constraints_result = grid_layout_cell_constraints(layout, o, ctx, cross_axis_size_override);
    let orientation_literal = llr_Expression::EnumerationValue(EnumerationValue {
        value: o as _,
        enumeration: crate::typeregister::BUILTIN.enums.Orientation.clone(),
    });

    let sub_expression = llr_Expression::ExtraBuiltinFunctionCall {
        function: "grid_layout_info".into(),
        arguments: vec![
            llr_Expression::PropertyReference(organized_cells),
            constraints_result.cells,
            if constraints_result.compute_cells.is_none() {
                empty_int32_slice()
            } else {
                llr_Expression::ReadLocalVariable {
                    name: "repeated_indices".into(),
                    ty: Type::Array(Type::Int32.into()),
                }
            },
            if constraints_result.compute_cells.is_none() {
                empty_int32_slice()
            } else {
                llr_Expression::ReadLocalVariable {
                    name: "repeater_steps".into(),
                    ty: Type::Array(Type::Int32.into()),
                }
            },
            spacing,
            padding,
            orientation_literal,
        ],
        return_ty: crate::typeregister::layout_info_type().into(),
    };
    match constraints_result.compute_cells {
        Some((cells_variable, elements)) => llr_Expression::WithLayoutItemInfo {
            cells_variable,
            repeater_indices_var_name: Some("repeated_indices".into()),
            repeater_steps_var_name: Some("repeater_steps".into()),
            elements,
            orientation: o,
            repeated_cross_size: None,
            sub_expression: Box::new(sub_expression),
        },
        None => sub_expression,
    }
}

/// Whether a repeated cell of `layout` measures the `o` axis through a
/// parametrized layout-info function (height-for-width for Vertical,
/// width-for-height for Horizontal). Only then does forwarding a cross-axis
/// size to the repeated cells change anything.
fn box_layout_has_constrained_repeated_cell(
    layout: &crate::layout::BoxLayout,
    o: Orientation,
) -> bool {
    layout.elems.iter().any(|item| {
        item.element.borrow().repeated.is_some()
            && match o {
                Orientation::Vertical => {
                    item.element.borrow().has_inherited_layout_info_v_with_constraint()
                }
                Orientation::Horizontal => {
                    item.element.borrow().has_inherited_layout_info_h_with_constraint()
                }
            }
    })
}

/// Name of the local that carries the known width (resp. height) a measure
/// pass measures a cell at. The generated code binds it around each static
/// measure cell's `LayoutInfo` expression; shared by the flexbox and box
/// layout measure passes.
pub const MEASURE_KNOWN_W_LOCAL: &str = "measure_known_w";
/// See [`MEASURE_KNOWN_W_LOCAL`].
pub const MEASURE_KNOWN_H_LOCAL: &str = "measure_known_h";

pub(super) fn compute_box_layout_info(
    layout: &crate::layout::BoxLayout,
    o: Orientation,
    ctx: &mut ExpressionLoweringCtx,
    cross_axis_size_override: Option<&crate::expression_tree::Expression>,
) -> llr_Expression {
    let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
    // Cross-axis info at a known main-axis size (a horizontal layout's
    // vertical info at a known width, or the mirror): solve the main axis at
    // that size and measure each height-for-width (resp. width-for-height)
    // cell at its solved main size — feeding every cell the whole size would
    // overestimate what the layout actually gives it, and so underestimate
    // the cross size the cell needs.
    if o != layout.orientation
        && let Some(override_expr) = cross_axis_size_override
        && box_layout_needs_measure(layout, o)
    {
        return compute_box_layout_info_ortho_with_measure(layout, ctx, override_expr, padding, o);
    }
    let adjusted_override = cross_axis_size_override
        .map(|o_expr| subtract_padding(o_expr.clone(), &layout.geometry, o.orthogonal()));
    let bld = box_layout_data(layout, o, ctx, adjusted_override.as_ref(), None, false, false);
    let sub_expression = if o == layout.orientation {
        llr_Expression::ExtraBuiltinFunctionCall {
            function: "box_layout_info".into(),
            arguments: vec![bld.cells, spacing, padding, bld.alignment],
            return_ty: crate::typeregister::layout_info_type().into(),
        }
    } else {
        llr_Expression::ExtraBuiltinFunctionCall {
            function: "box_layout_info_ortho".into(),
            arguments: vec![bld.cells, padding],
            return_ty: crate::typeregister::layout_info_type().into(),
        }
    };
    // On the main pass with a known cross-axis size (a `layoutinfo-*-with-constraint`
    // body), measure repeated cells at that size too, like the static cells.
    let repeated_cross_size = adjusted_override
        .as_ref()
        .filter(|_| o == layout.orientation && box_layout_has_constrained_repeated_cell(layout, o))
        .map(|e| Box::new(super::lower_expression::lower_expression(e, ctx)));
    match bld.compute_cells {
        Some((cells_variable, elements)) => llr_Expression::WithLayoutItemInfo {
            cells_variable,
            repeater_indices_var_name: None,
            repeater_steps_var_name: None,
            elements,
            orientation: o,
            repeated_cross_size,
            sub_expression: Box::new(sub_expression),
        },
        None => sub_expression,
    }
}

/// Whether any cell of the box layout is height-for-width (`o` Vertical) resp.
/// width-for-height (`o` Horizontal), so an `o`-axis info computation at a
/// known main-axis size needs the solve-and-measure pass.
fn box_layout_needs_measure(layout: &crate::layout::BoxLayout, o: Orientation) -> bool {
    layout.elems.iter().any(|li| {
        let (h4w, w4h) = cell_measure_capability(&li.element);
        match o {
            Orientation::Vertical => h4w,
            Orientation::Horizontal => w4h,
        }
    })
}

/// Per-element measure inputs for [`llr_Expression::BoxLayoutInfoOrthoWithMeasure`]:
/// the cell's `o`-axis `LayoutInfo` measured at its solved main size, read from
/// the [`MEASURE_KNOWN_W_LOCAL`] resp. [`MEASURE_KNOWN_H_LOCAL`] local (the
/// same locals the flexbox measure cells use). A repeated element becomes a
/// [`BoxMeasureCell::Repeated`]: its instances are only known at solve time, so
/// the generated code queries each instance's `layout_item_info_at_cross_width`
/// / `_at_cross_height` directly.
fn box_measure_cells_for(
    layout: &crate::layout::BoxLayout,
    ctx: &mut ExpressionLoweringCtx,
    o: Orientation,
) -> Vec<BoxMeasureCell> {
    layout
        .elems
        .iter()
        .map(|li| {
            let elem = &li.element;
            if elem.borrow().repeated.is_some() {
                let repeater_index =
                    match ctx.mapping.element_mapping.get(&elem.clone().into()).unwrap() {
                        LoweredElement::Repeated { repeated_index } => *repeated_index,
                        _ => panic!("repeated box layout element not lowered as Repeated"),
                    };
                return BoxMeasureCell::Repeated(LayoutRepeatedElement {
                    repeater_index,
                    row_child_templates: None,
                    cross_width: None,
                });
            }
            let measure_local = crate::expression_tree::Expression::ReadLocalVariable {
                name: match o {
                    Orientation::Vertical => MEASURE_KNOWN_W_LOCAL.into(),
                    Orientation::Horizontal => MEASURE_KNOWN_H_LOCAL.into(),
                },
                ty: Type::LogicalLength,
            };
            let info =
                cell_layout_info(elem, &li.constraints, ctx, o, Some(&measure_local), None, false);
            BoxMeasureCell::Static { info }
        })
        .collect()
}

/// Build the [`llr_Expression::BoxLayoutInfoOrthoWithMeasure`] for the layout's
/// `o`-axis info at the known main-axis size `cross_axis_size` (a horizontal
/// layout's vertical info at a known width, or the mirror).
fn compute_box_layout_info_ortho_with_measure(
    layout: &crate::layout::BoxLayout,
    ctx: &mut ExpressionLoweringCtx,
    cross_axis_size: &crate::expression_tree::Expression,
    padding_ortho: llr_Expression,
    o: Orientation,
) -> llr_Expression {
    let main_o = layout.orientation;
    let (padding_main, spacing_main) =
        generate_layout_padding_and_spacing(&layout.geometry, main_o, ctx);
    let bld = box_layout_data(layout, main_o, ctx, None, None, false, true);
    let size = super::lower_expression::lower_expression(cross_axis_size, ctx);
    let solve_data = make_struct(
        BuiltinStruct::BoxLayoutData,
        [
            ("size", Type::Float32, size),
            ("spacing", Type::Float32, spacing_main),
            ("padding", padding_main.ty(ctx), padding_main),
            (
                "alignment",
                Type::Enumeration(crate::typeregister::BUILTIN.enums.LayoutAlignment.clone()),
                bld.alignment,
            ),
            ("cells", bld.cells.ty(ctx), bld.cells),
        ],
    );
    let sub_expression = llr_Expression::BoxLayoutInfoOrthoWithMeasure {
        solve_data: Box::new(solve_data),
        padding_ortho: Box::new(padding_ortho),
        orientation: o,
        measure_cells: box_measure_cells_for(layout, ctx, o),
    };
    match bld.compute_cells {
        Some((cells_variable, elements)) => llr_Expression::WithLayoutItemInfo {
            cells_variable,
            repeater_indices_var_name: None,
            repeater_steps_var_name: None,
            elements,
            orientation: main_o,
            repeated_cross_size: None,
            sub_expression: Box::new(sub_expression),
        },
        None => sub_expression,
    }
}

pub(super) fn organize_grid_layout(
    layout: &crate::layout::GridLayout,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    let input_data = grid_layout_input_data(layout, ctx);

    if let Some(button_roles) = &layout.dialog_button_roles {
        let e = crate::typeregister::BUILTIN.enums.DialogButtonRole.clone();
        let roles = button_roles
            .iter()
            .map(|r| {
                llr_Expression::EnumerationValue(EnumerationValue {
                    value: e.values.iter().position(|x| x == r).unwrap() as _,
                    enumeration: e.clone(),
                })
            })
            .collect();
        let roles_expr = llr_Expression::Array {
            element_ty: Type::Enumeration(e),
            values: roles,
            output: llr_ArrayOutput::Slice,
        };
        llr_Expression::ExtraBuiltinFunctionCall {
            function: "organize_dialog_button_layout".into(),
            arguments: vec![input_data.cells, roles_expr],
            return_ty: Type::Array(Type::Int32.into()),
        }
    } else {
        let sub_expression = llr_Expression::ExtraBuiltinFunctionCall {
            function: "organize_grid_layout".into(),
            arguments: vec![
                input_data.cells,
                if input_data.compute_cells.is_none() {
                    empty_int32_slice()
                } else {
                    llr_Expression::ReadLocalVariable {
                        name: SmolStr::new_static("repeated_indices"),
                        ty: Type::Array(Type::Int32.into()),
                    }
                },
                if input_data.compute_cells.is_none() {
                    empty_int32_slice()
                } else {
                    llr_Expression::ReadLocalVariable {
                        name: SmolStr::new_static("repeater_steps"),
                        ty: Type::Array(Type::Int32.into()),
                    }
                },
            ],
            return_ty: Type::Array(Type::Int32.into()),
        };
        if let Some((cells_variable, elements)) = input_data.compute_cells {
            llr_Expression::WithGridInputData {
                cells_variable,
                repeater_indices_var_name: SmolStr::new_static("repeated_indices"),
                repeater_steps_var_name: SmolStr::new_static("repeater_steps"),
                elements,
                sub_expression: Box::new(sub_expression),
            }
        } else {
            sub_expression
        }
    }
}

pub(super) fn solve_grid_layout(
    layout_organized_data_prop: &NamedReference,
    layout: &crate::layout::GridLayout,
    o: Orientation,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
    let cells = ctx.map_property_reference(layout_organized_data_prop);
    let size = layout_geometry_size(&layout.geometry.rect, o, ctx);
    let orientation_expr = llr_Expression::EnumerationValue(EnumerationValue {
        value: o as _,
        enumeration: crate::typeregister::BUILTIN.enums.Orientation.clone(),
    });
    let data = make_struct(
        BuiltinStruct::GridLayoutData,
        [
            ("size", Type::Float32, size),
            ("spacing", Type::Float32, spacing),
            ("padding", padding.ty(ctx), padding),
            ("organized_data", Type::ArrayOfU16, llr_Expression::PropertyReference(cells)),
        ],
    );
    let constraints_result = grid_layout_cell_constraints(layout, o, ctx, None);

    match constraints_result.compute_cells {
        Some((cells_variable, elements)) => llr_Expression::WithLayoutItemInfo {
            cells_variable: cells_variable.clone(),
            repeater_indices_var_name: Some("repeated_indices".into()),
            repeater_steps_var_name: Some("repeater_steps".into()),
            elements,
            orientation: o,
            repeated_cross_size: None,
            sub_expression: Box::new(llr_Expression::ExtraBuiltinFunctionCall {
                function: "solve_grid_layout".into(),
                arguments: vec![
                    data,
                    llr_Expression::ReadLocalVariable {
                        name: cells_variable.into(),
                        ty: constraints_result.cells.ty(ctx),
                    },
                    orientation_expr,
                    llr_Expression::ReadLocalVariable {
                        name: "repeated_indices".into(),
                        ty: Type::Array(Type::Int32.into()),
                    },
                    llr_Expression::ReadLocalVariable {
                        name: "repeater_steps".into(),
                        ty: Type::Array(Type::Int32.into()),
                    },
                ],
                return_ty: Type::LayoutCache,
            }),
        },
        None => llr_Expression::ExtraBuiltinFunctionCall {
            function: "solve_grid_layout".into(),
            arguments: vec![
                data,
                constraints_result.cells,
                orientation_expr,
                empty_int32_slice(),
                empty_int32_slice(),
            ],
            return_ty: Type::LayoutCache,
        },
    }
}

pub(super) fn solve_box_layout(
    layout: &crate::layout::BoxLayout,
    o: Orientation,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
    // For a horizontal layout's main (width) pass, feed each width-for-height
    // child the layout's real cross size (its content height) instead of the
    // `f32::MAX` "assume infinite height" fallback, so the width reserved for the
    // child matches the height it will actually be given. The vertical main
    // pass must NOT do the mirror image: embedding `self.width` into the
    // cache would let a geometry pull inside a horizontal info chain (a cell
    // with `width: self.height`) close a binding loop through an ancestor's
    // cache. Height-for-width children read their own laid-out width instead
    // (see `text_layout_info` in i-slint-core).
    let cross_override = (o == layout.orientation && o == Orientation::Horizontal)
        .then(|| layout_cross_content_size(layout))
        .flatten();
    // On the cross pass, the layout's content size along `o` is its
    // cross content size; forward it so a wrapping perpendicular flex cell
    // gets its natural single-line size instead of the compact sqrt preferred.
    let cross_clamp =
        (o != layout.orientation).then(|| layout_cross_content_size(layout)).flatten();
    let bld =
        box_layout_data(layout, o, ctx, cross_override.as_ref(), cross_clamp.as_ref(), true, false);
    // On the main pass, measure repeated height-for-width (resp. width-for-height)
    // cells at the layout's cross content size, like the flexbox solve does: their
    // plain layout-info is measured at their preferred size, which is not the size
    // the layout gives them.
    let repeated_cross_size = (o == layout.orientation)
        .then(|| layout_cross_content_size(layout))
        .flatten()
        .filter(|_| box_layout_has_constrained_repeated_cell(layout, o))
        .map(|e| Box::new(super::lower_expression::lower_expression(&e, ctx)));
    let size = layout_geometry_size(&layout.geometry.rect, o, ctx);
    let (data, function) = if o == layout.orientation {
        let data = make_struct(
            BuiltinStruct::BoxLayoutData,
            [
                ("size", Type::Float32, size),
                ("spacing", Type::Float32, spacing),
                ("padding", padding.ty(ctx), padding),
                (
                    "alignment",
                    Type::Enumeration(crate::typeregister::BUILTIN.enums.LayoutAlignment.clone()),
                    bld.alignment,
                ),
                ("cells", bld.cells.ty(ctx), bld.cells),
            ],
        );
        (data, "solve_box_layout")
    } else {
        let cross_axis_alignment_ty =
            Type::Enumeration(crate::typeregister::BUILTIN.enums.CrossAxisAlignment.clone());
        let cross_axis_alignment = if let Some(nr) = &layout.cross_alignment {
            llr_Expression::PropertyReference(ctx.map_property_reference(nr))
        } else {
            let e = crate::typeregister::BUILTIN.enums.CrossAxisAlignment.clone();
            llr_Expression::EnumerationValue(EnumerationValue {
                value: e.default_value,
                enumeration: e,
            })
        };
        let data = make_struct(
            BuiltinStruct::BoxLayoutOrthoData,
            [
                ("size", Type::Float32, size),
                ("padding", padding.ty(ctx), padding),
                ("cross_axis_alignment", cross_axis_alignment_ty, cross_axis_alignment),
                ("cells", bld.cells.ty(ctx), bld.cells),
            ],
        );
        (data, "solve_box_layout_ortho")
    };
    match bld.compute_cells {
        Some((cells_variable, elements)) => llr_Expression::WithLayoutItemInfo {
            cells_variable,
            repeater_indices_var_name: Some("repeated_indices".into()),
            repeater_steps_var_name: None,
            elements,
            orientation: o,
            repeated_cross_size,
            sub_expression: Box::new(llr_Expression::ExtraBuiltinFunctionCall {
                function: function.into(),
                arguments: vec![
                    data,
                    llr_Expression::ReadLocalVariable {
                        name: "repeated_indices".into(),
                        ty: Type::Array(Type::Int32.into()),
                    },
                ],
                return_ty: Type::LayoutCache,
            }),
        },
        None => llr_Expression::ExtraBuiltinFunctionCall {
            function: function.into(),
            arguments: vec![data, empty_int32_slice()],
            return_ty: Type::LayoutCache,
        },
    }
}

pub(super) fn solve_flexbox_layout(
    layout: &crate::layout::FlexboxLayout,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    let (padding_h, spacing_h) =
        generate_layout_padding_and_spacing(&layout.geometry, Orientation::Horizontal, ctx);
    let (padding_v, spacing_v) =
        generate_layout_padding_and_spacing(&layout.geometry, Orientation::Vertical, ctx);
    // At solve time, the container width is known (set by our parent).
    // For column-direction flex (vertical main axis), each cell is
    // at most as wide as the container (per-column when wrapped), an upper
    // bound to supply as the cross-axis constraint to height-for-width children.
    let container_width_for_cells = if matches!(
        layout.axis_relation(Orientation::Vertical),
        crate::layout::FlexboxAxisRelation::MainAxis
    ) {
        layout.geometry.rect.width_reference.as_ref().map(|nr| {
            subtract_padding(
                crate::expression_tree::Expression::PropertyReference(nr.clone()),
                &layout.geometry,
                Orientation::Horizontal,
            )
        })
    } else {
        None
    };
    let fld = flexbox_layout_data(layout, ctx, container_width_for_cells.as_ref(), None);
    let width = layout_geometry_size(&layout.geometry.rect, Orientation::Horizontal, ctx);
    let height = layout_geometry_size(&layout.geometry.rect, Orientation::Vertical, ctx);
    let data = make_struct(
        BuiltinStruct::FlexboxLayoutData,
        [
            ("width", Type::Float32, width),
            ("height", Type::Float32, height),
            ("spacing_h", Type::Float32, spacing_h),
            ("spacing_v", Type::Float32, spacing_v),
            ("padding_h", padding_h.ty(ctx), padding_h),
            ("padding_v", padding_v.ty(ctx), padding_v),
            (
                "alignment",
                Type::Enumeration(crate::typeregister::BUILTIN.enums.LayoutAlignment.clone()),
                fld.alignment,
            ),
            (
                "direction",
                Type::Enumeration(
                    crate::typeregister::BUILTIN.enums.FlexboxLayoutDirection.clone(),
                ),
                fld.direction,
            ),
            (
                "cross_axis_line_alignment",
                Type::Enumeration(crate::typeregister::BUILTIN.enums.LayoutAlignment.clone()),
                fld.cross_axis_line_alignment,
            ),
            (
                "cross_axis_alignment",
                Type::Enumeration(crate::typeregister::BUILTIN.enums.CrossAxisAlignment.clone()),
                fld.cross_axis_alignment,
            ),
            (
                "flex_wrap",
                Type::Enumeration(crate::typeregister::BUILTIN.enums.FlexboxLayoutWrap.clone()),
                fld.flex_wrap,
            ),
            ("cells_h", fld.cells_h.ty(ctx), fld.cells_h),
            ("cells_v", fld.cells_v.ty(ctx), fld.cells_v),
            ("flex_props", fld.flex_props.ty(ctx), fld.flex_props),
        ],
    );
    // Forward the container width to repeated cells so a column flex re-measures
    // each height-for-width instance at the real width (parity with static cells,
    // which use the same `width_override`). `None` for a row flex.
    let repeated_cross_width = container_width_for_cells
        .as_ref()
        .map(|e| Box::new(super::lower_expression::lower_expression(e, ctx)));
    // Only height-for-width-capable cells benefit from re-measuring;
    // a flexbox without any keeps the cheaper plain solve.
    let needs_measure = flexbox_needs_measure(layout);
    match fld.compute_cells {
        Some((cells_h_var, cells_v_var, flex_var, elements)) => {
            let repeated_indices = || llr_Expression::ReadLocalVariable {
                name: "repeated_indices".into(),
                ty: Type::Array(Type::Int32.into()),
            };
            let sub_expression = if needs_measure {
                llr_Expression::SolveFlexboxLayoutWithMeasure {
                    data: Box::new(data),
                    repeater_indices: Box::new(repeated_indices()),
                    measure_cells: measure_cells_for(layout, ctx),
                }
            } else {
                llr_Expression::ExtraBuiltinFunctionCall {
                    function: "solve_flexbox_layout".into(),
                    arguments: vec![data, repeated_indices()],
                    return_ty: Type::LayoutCache,
                }
            };
            llr_Expression::WithFlexboxLayoutItemInfo {
                cells_h_variable: cells_h_var,
                cells_v_variable: cells_v_var,
                flex_props_variable: Some(flex_var),
                repeater_indices_var_name: Some("repeated_indices".into()),
                elements,
                repeated_cross_width,
                sub_expression: Box::new(sub_expression),
            }
        }
        None => {
            if !needs_measure {
                return llr_Expression::ExtraBuiltinFunctionCall {
                    function: "solve_flexbox_layout".into(),
                    arguments: vec![data, empty_int32_slice()],
                    return_ty: Type::LayoutCache,
                };
            }
            llr_Expression::SolveFlexboxLayoutWithMeasure {
                data: Box::new(data),
                repeater_indices: Box::new(empty_int32_slice()),
                measure_cells: measure_cells_for(layout, ctx),
            }
        }
    }
}

/// Whether the cell's vertical info depends on the width (height-for-width)
/// and its horizontal info on the height (width-for-height). For a repeater,
/// check the repeated component's root: that is the element the measure
/// callback queries.
fn cell_measure_capability(elem: &ElementRc) -> (bool, bool) {
    if elem.borrow().repeated.is_some() {
        let root = elem.borrow().base_type.as_component().root_element.clone();
        let h4w = root.borrow().inherited_layout_info_v_with_constraint().is_some();
        let w4h = root.borrow().inherited_layout_info_h_with_constraint().is_some();
        return (h4w, w4h);
    }
    (
        is_height_for_width_cell(elem),
        elem.borrow().inherited_layout_info_h_with_constraint().is_some(),
    )
}

/// Whether any cell of the flexbox is height-for-width (or width-for-height)
/// capable, so a solve or cross-axis info computation needs a measure callback.
fn flexbox_needs_measure(layout: &crate::layout::FlexboxLayout) -> bool {
    layout.elems.iter().any(|li| {
        let (h4w, w4h) = cell_measure_capability(&li.element);
        h4w || w4h
    })
}

/// Per-element measure inputs for `SolveFlexboxLayoutWithMeasure` and
/// `FlexboxLayoutInfoCrossAxisWithMeasure`: the cell's `(h_info, v_info)`
/// measured at the dimension taffy assigns, read from the `measure_known_w` /
/// `measure_known_h` locals. A repeated element becomes a
/// `FlexboxMeasureCellKind::Repeated`: its instances are only known at solve
/// time, so the generated callback queries the instance directly. A static
/// element with no constrained layout info becomes a
/// `FlexboxMeasureCellKind::Fixed` and gets no measure arm.
fn measure_cells_for(
    layout: &crate::layout::FlexboxLayout,
    ctx: &mut ExpressionLoweringCtx,
) -> Vec<FlexboxMeasureCell> {
    layout
        .elems
        .iter()
        .map(|li| {
            let elem = &li.element;
            if elem.borrow().repeated.is_some() {
                let (h4w, w4h) = cell_measure_capability(elem);
                let w4h_only = w4h && !h4w;
                let repeater_index =
                    match ctx.mapping.element_mapping.get(&elem.clone().into()).unwrap() {
                        LoweredElement::Repeated { repeated_index } => *repeated_index,
                        _ => panic!("repeated flexbox element not lowered as Repeated"),
                    };
                return FlexboxMeasureCell {
                    kind: FlexboxMeasureCellKind::Repeated(LayoutRepeatedElement {
                        repeater_index,
                        row_child_templates: None,
                        cross_width: None,
                    }),
                    w4h_only,
                };
            }
            let (h4w, w4h) = cell_measure_capability(elem);
            if !h4w && !w4h {
                return FlexboxMeasureCell { kind: FlexboxMeasureCellKind::Fixed, w4h_only: false };
            }
            let v_constraint = h4w.then(|| crate::expression_tree::Expression::ReadLocalVariable {
                name: MEASURE_KNOWN_W_LOCAL.into(),
                ty: Type::LogicalLength,
            });
            let v_info = get_flex_cell_layout_info(
                elem,
                ctx,
                &li.constraints,
                Orientation::Vertical,
                v_constraint,
            );
            let h_constraint = w4h.then(|| crate::expression_tree::Expression::ReadLocalVariable {
                name: MEASURE_KNOWN_H_LOCAL.into(),
                ty: Type::LogicalLength,
            });
            let h_info = get_flex_cell_layout_info(
                elem,
                ctx,
                &li.constraints,
                Orientation::Horizontal,
                h_constraint,
            );
            FlexboxMeasureCell {
                kind: FlexboxMeasureCellKind::Static { h_info, v_info },
                w4h_only: w4h && !h4w,
            }
        })
        .collect()
}

pub(super) fn compute_flexbox_layout_info(
    layout: &crate::layout::FlexboxLayout,
    orientation: Orientation,
    ctx: &mut ExpressionLoweringCtx,
    cross_axis_size_override: Option<&crate::expression_tree::Expression>,
) -> llr_Expression {
    // The override carries a width when called from a
    // `layoutinfo-v-with-constraint` body, a height when called from a
    // `layoutinfo-h-with-constraint` body. Route it to the matching
    // cell-list so cells don't receive a dimension on the wrong axis.
    let (width_override, height_override) = match orientation {
        Orientation::Vertical => (cross_axis_size_override, None),
        Orientation::Horizontal => (None, cross_axis_size_override),
    };
    // Subtract padding so height-for-width cells are measured at the content
    // width they are actually laid out at, not the padded outer width.
    let width_override = width_override
        .map(|e| subtract_padding(e.clone(), &layout.geometry, Orientation::Horizontal));
    let height_override = height_override
        .map(|e| subtract_padding(e.clone(), &layout.geometry, Orientation::Vertical));
    let fld = flexbox_layout_data(layout, ctx, width_override.as_ref(), height_override.as_ref());

    match layout.axis_relation(orientation) {
        crate::layout::FlexboxAxisRelation::MainAxis => {
            compute_flexbox_layout_info_for_direction(layout, orientation, false, fld, ctx, None)
        }
        crate::layout::FlexboxAxisRelation::CrossAxis => compute_flexbox_layout_info_for_direction(
            layout,
            orientation,
            true,
            fld,
            ctx,
            cross_axis_size_override,
        ),
        crate::layout::FlexboxAxisRelation::Unknown => {
            // Direction is not known at compile time - generate runtime conditional
            // This ensures we only read the constraint (width/height) in the branch where it's needed
            let row_expr = compute_flexbox_layout_info_for_direction(
                layout,
                orientation,
                orientation == Orientation::Vertical, // cross-axis if orientation is vertical
                fld.clone(),
                ctx,
                cross_axis_size_override,
            );
            let col_expr = compute_flexbox_layout_info_for_direction(
                layout,
                orientation,
                orientation == Orientation::Horizontal, // cross-axis if orientation is horizontal
                fld,
                ctx,
                cross_axis_size_override,
            );

            // Condition: direction == Row || direction == RowReverse
            let direction_enum = crate::typeregister::BUILTIN.enums.FlexboxLayoutDirection.clone();
            let direction_ref = llr_Expression::PropertyReference(
                ctx.map_property_reference(layout.direction.as_ref().unwrap()),
            );

            let is_row_condition = llr_Expression::BinaryExpression {
                lhs: Box::new(llr_Expression::BinaryExpression {
                    lhs: Box::new(direction_ref.clone()),
                    rhs: Box::new(llr_Expression::EnumerationValue(EnumerationValue {
                        value: 0, // FlexboxLayoutDirection::Row
                        enumeration: direction_enum.clone(),
                    })),
                    op: '=',
                }),
                rhs: Box::new(llr_Expression::BinaryExpression {
                    lhs: Box::new(direction_ref),
                    rhs: Box::new(llr_Expression::EnumerationValue(EnumerationValue {
                        value: 1, // FlexboxLayoutDirection::RowReverse
                        enumeration: direction_enum,
                    })),
                    op: '=',
                }),
                op: '|',
            };

            llr_Expression::Condition {
                condition: Box::new(is_row_condition),
                true_expr: Box::new(row_expr),
                false_expr: Box::new(col_expr),
            }
        }
    }
}

fn compute_flexbox_layout_info_for_direction(
    layout: &crate::layout::FlexboxLayout,
    orientation: Orientation,
    is_cross_axis: bool,
    fld: FlexboxLayoutDataResult,
    ctx: &mut ExpressionLoweringCtx,
    cross_axis_size_override: Option<&crate::expression_tree::Expression>,
) -> llr_Expression {
    let (padding_h, spacing_h) =
        generate_layout_padding_and_spacing(&layout.geometry, Orientation::Horizontal, ctx);
    let (padding_v, spacing_v) =
        generate_layout_padding_and_spacing(&layout.geometry, Orientation::Vertical, ctx);

    if is_cross_axis {
        // Cross-axis layout info: pass the main-axis container dimension
        // as constraint for accurate wrapping. The override (when set)
        // replaces a `self.{width,height}` read that would otherwise
        // cycle if this flex is nested on the perpendicular axis.
        let constraint_size = if let Some(override_expr) = cross_axis_size_override {
            super::lower_expression::lower_expression(override_expr, ctx)
        } else {
            match orientation {
                Orientation::Horizontal => {
                    layout_geometry_size(&layout.geometry.rect, Orientation::Vertical, ctx)
                }
                Orientation::Vertical => {
                    layout_geometry_size(&layout.geometry.rect, Orientation::Horizontal, ctx)
                }
            }
        };

        let arguments = vec![
            fld.cells_h,
            fld.cells_v,
            fld.flex_props,
            spacing_h,
            spacing_v,
            padding_h,
            padding_v,
            fld.direction,
            // Under `alignment: stretch` the solve grows the cells along the
            // main axis, changing a height-for-width cell's cross size, so
            // this measurement must apply the same growth.
            fld.alignment,
            fld.flex_wrap,
            constraint_size,
        ];

        // Re-measure height-for-width cells at the main-axis size taffy
        // assigns them, not at the container size the cells in `arguments`
        // were pre-measured at: e.g. a nested wrapping flexbox laid out at
        // its preferred width can be taller than when given the full
        // container width.
        let sub_expression = if flexbox_needs_measure(layout) {
            llr_Expression::FlexboxLayoutInfoCrossAxisWithMeasure {
                arguments,
                measure_cells: measure_cells_for(layout, ctx),
            }
        } else {
            llr_Expression::ExtraBuiltinFunctionCall {
                function: "flexbox_layout_info_cross_axis".into(),
                arguments,
                return_ty: crate::typeregister::layout_info_type().into(),
            }
        };
        match fld.compute_cells {
            Some((cells_h_var, cells_v_var, flex_var, elements)) => {
                llr_Expression::WithFlexboxLayoutItemInfo {
                    cells_h_variable: cells_h_var,
                    cells_v_variable: cells_v_var,
                    flex_props_variable: Some(flex_var),
                    repeater_indices_var_name: None,
                    elements,
                    // Info computation, not a solve: no container width to forward.
                    repeated_cross_width: None,
                    sub_expression: Box::new(sub_expression),
                }
            }
            None => sub_expression,
        }
    } else {
        // Main axis: only needs same-axis cells, avoiding cross-axis binding loop.
        let (cells, spacing, padding) = match orientation {
            Orientation::Horizontal => (fld.cells_h, spacing_h, padding_h),
            Orientation::Vertical => (fld.cells_v, spacing_v, padding_v),
        };

        match fld.compute_cells {
            Some((cells_h_var, cells_v_var, _flex_var, elements)) => {
                let cells_var = match orientation {
                    Orientation::Horizontal => cells_h_var.clone(),
                    Orientation::Vertical => cells_v_var.clone(),
                };
                llr_Expression::WithFlexboxLayoutItemInfo {
                    cells_h_variable: cells_h_var,
                    cells_v_variable: cells_v_var,
                    flex_props_variable: None,
                    repeater_indices_var_name: None,
                    elements,
                    // Info computation, not a solve: no container width to forward.
                    repeated_cross_width: None,
                    sub_expression: Box::new(llr_Expression::ExtraBuiltinFunctionCall {
                        function: "flexbox_layout_info_main_axis".into(),
                        arguments: vec![
                            llr_Expression::ReadLocalVariable {
                                name: cells_var.into(),
                                ty: Type::Array(Arc::new(
                                    crate::typeregister::layout_item_info_type(),
                                )),
                            },
                            spacing,
                            padding,
                            fld.flex_wrap,
                        ],
                        return_ty: crate::typeregister::layout_info_type().into(),
                    }),
                }
            }
            None => llr_Expression::ExtraBuiltinFunctionCall {
                function: "flexbox_layout_info_main_axis".into(),
                arguments: vec![cells, spacing, padding, fld.flex_wrap],
                return_ty: crate::typeregister::layout_info_type().into(),
            },
        }
    }
}

#[derive(Clone)]
struct FlexboxLayoutDataResult {
    alignment: llr_Expression,
    direction: llr_Expression,
    cross_axis_line_alignment: llr_Expression,
    cross_axis_alignment: llr_Expression,
    flex_wrap: llr_Expression,
    cells_h: llr_Expression,
    cells_v: llr_Expression,
    /// Per-item flex properties, parallel to `cells_h`/`cells_v` (or read from
    /// the `flex_props` variable built by `WithFlexboxLayoutItemInfo`).
    flex_props: llr_Expression,
    /// When there are repeaters involved, we need to do a WithFlexboxLayoutItemInfo with the
    /// given cells_h/cells_v/flex_props variable names and elements (each static element
    /// has a tuple of (h constraint, v constraint, flex props))
    compute_cells: Option<(
        String,
        String,
        String,
        Vec<Either<(llr_Expression, llr_Expression, llr_Expression), LayoutRepeatedElement>>,
    )>,
}

fn flexbox_layout_data(
    layout: &crate::layout::FlexboxLayout,
    ctx: &mut ExpressionLoweringCtx,
    width_override: Option<&crate::expression_tree::Expression>,
    height_override: Option<&crate::expression_tree::Expression>,
) -> FlexboxLayoutDataResult {
    let alignment = if let Some(expr) = &layout.geometry.alignment {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.enums.LayoutAlignment.clone();
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let direction = if let Some(expr) = &layout.direction {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.enums.FlexboxLayoutDirection.clone();
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let cross_axis_line_alignment = if let Some(expr) = &layout.cross_axis_line_alignment {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.enums.LayoutAlignment.clone();
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let cross_axis_alignment = if let Some(expr) = &layout.cross_axis_alignment {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.enums.CrossAxisAlignment.clone();
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let flex_wrap = if let Some(expr) = &layout.flex_wrap {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.enums.FlexboxLayoutWrap.clone();
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let repeater_count =
        layout.elems.iter().filter(|i| i.element.borrow().repeated.is_some()).count();

    let cell_ty = crate::typeregister::layout_item_info_type();
    let flex_props_ty = crate::typeregister::flex_item_props_type();

    let flex_prop =
        |li: &crate::layout::LayoutItem, ctx: &mut ExpressionLoweringCtx| -> FlexItemProps {
            FlexItemProps {
                align_self: li
                    .cross_axis_self_alignment
                    .as_ref()
                    .map(|nr| llr_Expression::PropertyReference(ctx.map_property_reference(nr)))
                    .unwrap_or(default_align_self().1),
                order: li
                    .layout_order
                    .as_ref()
                    .map(|nr| llr_Expression::PropertyReference(ctx.map_property_reference(nr)))
                    .unwrap_or(llr_Expression::NumberLiteral(0.0)),
            }
        };

    // Width constraint for a cell's cells_v entry. Use the explicit
    // width-override when one is in scope (solve-time container width,
    // or width parameter of a synthesized `layoutinfo-v-with-constraint`
    // body); otherwise fall back to the element's own preferred
    // horizontal size. Cells that are not height-for-width get `None`.
    let cell_v_constraint = |elem: &ElementRc| -> Option<crate::expression_tree::Expression> {
        // A component that forwards a height-for-width layout (e.g. `min-height:
        // inner.min-height` over a wrapped Text) has a `layoutinfo-v-with-constraint`
        // but is not a builtin height-for-width cell. It must dispatch via that
        // function instead of reading its own width — which would cycle through the
        // flex solve. Mirror of `cell_h_constraint`.
        if elem.borrow().inherited_layout_info_v_with_constraint().is_some() {
            return Some(width_override.cloned().unwrap_or_else(|| {
                crate::expression_tree::Expression::NumberLiteral(
                    f32::MAX as f64,
                    crate::expression_tree::Unit::Px,
                )
            }));
        }
        if !is_height_for_width_cell(elem) {
            return None;
        }
        width_override.cloned().or_else(|| default_cross_axis_constraint(elem))
    };
    // Height constraint for a cell's cells_h entry. Dispatch via
    // `layoutinfo-h-with-constraint` for cells that have one. Use
    // `f32::MAX` ("unconstrained") when no explicit height-override is
    // in scope — that tells the runtime to treat the cell as not
    // needing to wrap, giving the natural max-cell-width rather than
    // the `sqrt(item-areas)` heuristic.
    let cell_h_constraint = |elem: &ElementRc| -> Option<crate::expression_tree::Expression> {
        if elem.borrow().inherited_layout_info_h_with_constraint().is_some() {
            Some(height_override.cloned().unwrap_or_else(|| {
                crate::expression_tree::Expression::NumberLiteral(
                    f32::MAX as f64,
                    crate::expression_tree::Unit::Px,
                )
            }))
        } else {
            None
        }
    };

    if repeater_count == 0 {
        let cells_h = llr_Expression::Array {
            values: layout
                .elems
                .iter()
                .map(|li| {
                    let constraint = cell_h_constraint(&li.element);
                    let layout_info_h = get_flex_cell_layout_info(
                        &li.element,
                        ctx,
                        &li.constraints,
                        Orientation::Horizontal,
                        constraint,
                    );
                    make_layout_cell_data_struct(layout_info_h, None, None)
                })
                .collect(),
            element_ty: cell_ty.clone(),
            output: llr_ArrayOutput::Slice,
        };
        // For cells_v, pass a width constraint for items that need
        // height-for-width (Text with word-wrap, Image with aspect ratio,
        // and components with a synthesized
        // `layoutinfo-v-with-constraint`).
        let cells_v = llr_Expression::Array {
            values: layout
                .elems
                .iter()
                .map(|li| {
                    let constraint = cell_v_constraint(&li.element);
                    let layout_info_v = get_flex_cell_layout_info(
                        &li.element,
                        ctx,
                        &li.constraints,
                        Orientation::Vertical,
                        constraint,
                    );
                    make_layout_cell_data_struct(layout_info_v, None, None)
                })
                .collect(),
            element_ty: cell_ty,
            output: llr_ArrayOutput::Slice,
        };
        let flex_props = llr_Expression::Array {
            values: layout
                .elems
                .iter()
                .map(|li| make_flex_props_struct(flex_prop(li, ctx)))
                .collect(),
            element_ty: flex_props_ty,
            output: llr_ArrayOutput::Slice,
        };
        FlexboxLayoutDataResult {
            alignment,
            direction,
            cross_axis_line_alignment,
            cross_axis_alignment,
            flex_wrap,
            cells_h,
            cells_v,
            flex_props,
            compute_cells: None,
        }
    } else {
        let mut elements = Vec::new();
        for item in &layout.elems {
            if item.element.borrow().repeated.is_some() {
                let repeater_index =
                    match ctx.mapping.element_mapping.get(&item.element.clone().into()).unwrap() {
                        LoweredElement::Repeated { repeated_index } => *repeated_index,
                        _ => panic!(),
                    };
                elements.push(Either::Right(LayoutRepeatedElement {
                    repeater_index,
                    row_child_templates: None,
                    cross_width: None,
                }))
            } else {
                // For static elements, we need both orientations
                let h_constraint = cell_h_constraint(&item.element);
                let layout_info_h = get_flex_cell_layout_info(
                    &item.element,
                    ctx,
                    &item.constraints,
                    Orientation::Horizontal,
                    h_constraint,
                );
                let constraint = cell_v_constraint(&item.element);
                let layout_info_v = get_flex_cell_layout_info(
                    &item.element,
                    ctx,
                    &item.constraints,
                    Orientation::Vertical,
                    constraint,
                );
                elements.push(Either::Left((
                    make_layout_cell_data_struct(layout_info_h, None, None),
                    make_layout_cell_data_struct(layout_info_v, None, None),
                    make_flex_props_struct(flex_prop(item, ctx)),
                )));
            }
        }
        let cells_h = llr_Expression::ReadLocalVariable {
            name: "cells_h".into(),
            ty: Type::Array(Arc::new(crate::typeregister::layout_item_info_type())),
        };
        let cells_v = llr_Expression::ReadLocalVariable {
            name: "cells_v".into(),
            ty: Type::Array(Arc::new(crate::typeregister::layout_item_info_type())),
        };
        let flex_props = llr_Expression::ReadLocalVariable {
            name: "flex_props".into(),
            ty: Type::Array(Arc::new(crate::typeregister::flex_item_props_type())),
        };
        FlexboxLayoutDataResult {
            alignment,
            direction,
            cross_axis_line_alignment,
            cross_axis_alignment,
            flex_wrap,
            cells_h,
            cells_v,
            flex_props,
            compute_cells: Some((
                "cells_h".into(),
                "cells_v".into(),
                "flex_props".into(),
                elements,
            )),
        }
    }
}

struct BoxLayoutDataResult {
    alignment: llr_Expression,
    cells: llr_Expression,
    /// When there are repeater involved, we need to do a WithLayoutItemInfo with the
    /// given cell variable and elements
    compute_cells: Option<(String, Vec<Either<llr_Expression, LayoutRepeatedElement>>)>,
}

fn default_align_self() -> (Type, llr_Expression) {
    let e = crate::typeregister::BUILTIN.enums.CrossAxisSelfAlignment.clone();
    (
        Type::Enumeration(e.clone()),
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        }),
    )
}

/// Build a LayoutItemInfo struct expression with the canonical (full) field
/// list as its type, so the generators default the fields that are not set.
/// `align_self` is only set for a box layout's cross-axis cells, `order` only
/// for its main-axis ones.
fn make_layout_cell_data_struct(
    layout_info: llr_Expression,
    align_self: Option<llr_Expression>,
    order: Option<llr_Expression>,
) -> llr_Expression {
    let Type::Struct(ty) = crate::typeregister::layout_item_info_type() else { unreachable!() };
    let mut values = BTreeMap::<SmolStr, llr_Expression>::new();
    values.insert("constraint".into(), layout_info);
    if let Some(align_self) = align_self {
        values.insert("cross-axis-self-alignment".into(), align_self);
    }
    if let Some(order) = order {
        values.insert("layout-order".into(), order);
    }
    llr_Expression::Struct { ty, values }
}

#[derive(Clone)]
struct FlexItemProps {
    align_self: llr_Expression,
    order: llr_Expression,
}

fn make_flex_props_struct(fp: FlexItemProps) -> llr_Expression {
    let (align_self_ty, _) = default_align_self();
    make_struct(
        BuiltinStruct::FlexItemProps,
        [
            ("cross-axis-self-alignment", align_self_ty, fp.align_self),
            ("layout-order", Type::Int32, fp.order),
        ],
    )
}

fn box_layout_data(
    layout: &crate::layout::BoxLayout,
    orientation: Orientation,
    ctx: &mut ExpressionLoweringCtx,
    cross_axis_size_override: Option<&crate::expression_tree::Expression>,
    cross_clamp: Option<&crate::expression_tree::Expression>,
    for_solve: bool,
    for_measure_solve: bool,
) -> BoxLayoutDataResult {
    let alignment = if let Some(expr) = &layout.geometry.alignment {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.enums.LayoutAlignment.clone();
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let repeater_count =
        layout.elems.iter().filter(|i| i.element.borrow().repeated.is_some()).count();

    let element_ty = crate::typeregister::layout_item_info_type();

    // The per-item alignment only matters to the cross-axis solve. Leaving it
    // out of the main-axis cells keeps that cache independent of it, and out of
    // the layout-info cells because `box_layout_info_ortho` ignores it.
    // This covers static cells only: repeated cells go through the generated
    // `layout_item_info`, which is guarded on the orientation alone.
    let cell_align_self = |li: &crate::layout::LayoutItem, ctx: &mut ExpressionLoweringCtx| {
        li.cross_axis_self_alignment
            .as_ref()
            .filter(|_| for_solve && orientation != layout.orientation)
            .map(|nr| llr_Expression::PropertyReference(ctx.map_property_reference(nr)))
    };
    // `layout-order` is the mirror image: it only reorders the main-axis solve,
    // so keep it out of the cross-axis cache and of the layout-info cells (a
    // permutation changes neither the sum nor the merge of the constraints).
    let cell_order = |li: &crate::layout::LayoutItem, ctx: &mut ExpressionLoweringCtx| {
        li.layout_order
            .as_ref()
            .filter(|_| for_solve && orientation == layout.orientation)
            .map(|nr| llr_Expression::PropertyReference(ctx.map_property_reference(nr)))
    };
    if repeater_count == 0 {
        let cells = llr_Expression::Array {
            values: layout
                .elems
                .iter()
                .map(|li| {
                    let layout_info = cell_layout_info(
                        &li.element,
                        &li.constraints,
                        ctx,
                        orientation,
                        cross_axis_size_override,
                        cross_clamp,
                        for_measure_solve,
                    );
                    let align_self = cell_align_self(li, ctx);
                    let order = cell_order(li, ctx);
                    make_layout_cell_data_struct(layout_info, align_self, order)
                })
                .collect(),
            element_ty,
            output: llr_ArrayOutput::Slice,
        };
        BoxLayoutDataResult { alignment, cells, compute_cells: None }
    } else {
        let mut elements = Vec::new();
        for item in &layout.elems {
            if item.element.borrow().repeated.is_some() {
                let repeater_index =
                    match ctx.mapping.element_mapping.get(&item.element.clone().into()).unwrap() {
                        LoweredElement::Repeated { repeated_index } => *repeated_index,
                        _ => panic!(),
                    };
                elements.push(Either::Right(LayoutRepeatedElement {
                    repeater_index,
                    row_child_templates: None,
                    cross_width: None,
                }))
            } else {
                let layout_info = cell_layout_info(
                    &item.element,
                    &item.constraints,
                    ctx,
                    orientation,
                    cross_axis_size_override,
                    cross_clamp,
                    for_measure_solve,
                );
                let align_self = cell_align_self(item, ctx);
                let order = cell_order(item, ctx);
                elements.push(Either::Left(make_layout_cell_data_struct(
                    layout_info,
                    align_self,
                    order,
                )));
            }
        }
        let cells = llr_Expression::ReadLocalVariable {
            name: "cells".into(),
            ty: Type::Array(Arc::new(crate::typeregister::layout_info_type().into())),
        };
        BoxLayoutDataResult { alignment, cells, compute_cells: Some(("cells".into(), elements)) }
    }
}

/// `for_measure_solve` marks the main-axis solve inside
/// [`compute_box_layout_info_ortho_with_measure`]: height-for-width cells are
/// then measured at their preferred width rather than left unconstrained —
/// the unconstrained query reads the cell's current width, which can depend
/// on the very layout cache this solve is computed for.
fn cell_layout_info(
    elem: &ElementRc,
    constraints: &crate::layout::LayoutConstraints,
    ctx: &mut ExpressionLoweringCtx,
    orientation: Orientation,
    cross_axis_size_override: Option<&crate::expression_tree::Expression>,
    cross_clamp: Option<&crate::expression_tree::Expression>,
    for_measure_solve: bool,
) -> llr_Expression {
    let constraint = match orientation {
        Orientation::Vertical => cross_axis_size_override
            .filter(|_| is_height_for_width_cell(elem))
            .cloned()
            .or_else(|| {
                (for_measure_solve && is_height_for_width_cell(elem))
                    .then(|| default_cross_axis_constraint(elem))
                    .flatten()
            }),
        Orientation::Horizontal => {
            // Cells with `layoutinfo-h-with-constraint` need a constraint
            // to dispatch via the parametrized layout-info function
            // instead of reading the cell's own height — which would cycle
            // when the cell is a flex on the perpendicular
            // (horizontal-cross) axis.
            if elem.borrow().inherited_layout_info_h_with_constraint().is_some() {
                Some(cross_axis_size_override.cloned().unwrap_or_else(|| {
                    crate::expression_tree::Expression::NumberLiteral(
                        f32::MAX as f64,
                        crate::expression_tree::Unit::Px,
                    )
                }))
            } else {
                None
            }
        }
    };
    let layout_info = get_layout_info(elem, ctx, constraints, orientation, constraint);
    // On a box layout's cross pass (`cross_clamp` set), give a wrapping
    // perpendicular flex cell its natural single-line size clamped to the
    // available space instead of its compact sqrt preferred. An explicit
    // preferred size wins over the clamp, like in the interpreter (which applies
    // constraints after the clamp), so skip the clamp then.
    let has_explicit_preferred = match orientation {
        Orientation::Horizontal => constraints.preferred_width.is_some(),
        Orientation::Vertical => constraints.preferred_height.is_some(),
    };
    match cross_clamp {
        Some(available) if !has_explicit_preferred => {
            clamp_wrapping_flex_cross_preferred(layout_info, elem, orientation, available, ctx)
        }
        _ => layout_info,
    }
}

/// Build the `flexbox_layout_unwrapped_main(cells, spacing, padding)` call for
/// `layout`'s main axis (= `orientation`). Returns the flex's natural
/// single-line main size as a float expression.
fn flexbox_unwrapped_main_expr(
    layout: &crate::layout::FlexboxLayout,
    orientation: Orientation,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    let (padding_h, spacing_h) =
        generate_layout_padding_and_spacing(&layout.geometry, Orientation::Horizontal, ctx);
    let (padding_v, spacing_v) =
        generate_layout_padding_and_spacing(&layout.geometry, Orientation::Vertical, ctx);
    let fld = flexbox_layout_data(layout, ctx, None, None);
    let (spacing, padding) = match orientation {
        Orientation::Horizontal => (spacing_h, padding_h),
        Orientation::Vertical => (spacing_v, padding_v),
    };
    let cell_array_ty = Type::Array(Arc::new(crate::typeregister::layout_item_info_type()));
    let cells_expr = match &fld.compute_cells {
        Some((cells_h_var, cells_v_var, _, _)) => {
            let cells_var = match orientation {
                Orientation::Horizontal => cells_h_var.clone(),
                Orientation::Vertical => cells_v_var.clone(),
            };
            llr_Expression::ReadLocalVariable { name: cells_var.into(), ty: cell_array_ty }
        }
        None => match orientation {
            Orientation::Horizontal => fld.cells_h.clone(),
            Orientation::Vertical => fld.cells_v.clone(),
        },
    };
    let call = llr_Expression::ExtraBuiltinFunctionCall {
        function: "flexbox_layout_unwrapped_main".into(),
        arguments: vec![cells_expr, spacing, padding],
        return_ty: Type::Float32,
    };
    match fld.compute_cells {
        Some((cells_h_variable, cells_v_variable, _, elements)) => {
            llr_Expression::WithFlexboxLayoutItemInfo {
                cells_h_variable,
                cells_v_variable,
                // The call only reads the cells, so don't evaluate the
                // per-item flex properties (that would depend on them).
                flex_props_variable: None,
                repeater_indices_var_name: None,
                elements,
                // Info computation, not a solve: no container width to forward.
                repeated_cross_width: None,
                sub_expression: Box::new(call),
            }
        }
        None => call,
    }
}

/// If `elem` is a wrapping FlexboxLayout whose main axis is the parent's cross
/// axis (`orientation`), replace its `preferred` with
/// `min(available, unwrapped)`, where `unwrapped` is the flex's natural
/// single-line main size. Mirrors the interpreter's
/// `clamp_wrapping_flex_cross_preferred`. `available` is the layout's cross
/// content size (`layout_cross_content_size`).
fn clamp_wrapping_flex_cross_preferred(
    layout_info: llr_Expression,
    elem: &ElementRc,
    orientation: Orientation,
    available: &crate::expression_tree::Expression,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    let Some(flex) = crate::layout::FlexboxLayout::from_element(elem) else {
        return layout_info;
    };
    let axis_relation = flex.axis_relation(orientation);
    // The flex's main axis must be this cross axis. When the direction is known
    // at compile time to be the cross axis, there is nothing to clamp.
    if axis_relation == FlexboxAxisRelation::CrossAxis {
        return layout_info;
    }

    let unwrapped = flexbox_unwrapped_main_expr(&flex, orientation, ctx);
    let available = super::lower_expression::lower_expression(available, ctx);
    let clamped = llr_Expression::MinMax {
        ty: Type::Float32,
        op: MinMaxOp::Min,
        lhs: Box::new(available),
        rhs: Box::new(unwrapped),
    };

    // Rebuild the LayoutInfo struct, overriding only `preferred`.
    let ty = crate::typeregister::layout_info_type();
    let store = llr_Expression::StoreLocalVariable {
        name: "layout_info".into(),
        value: layout_info.into(),
    };
    let stored =
        || llr_Expression::ReadLocalVariable { name: "layout_info".into(), ty: ty.clone().into() };
    let stored_field = |name: &str| llr_Expression::StructFieldAccess {
        base: Box::new(stored()),
        name: name.into(),
    };
    // A no-wrap flex keeps its preferred (single line == its preferred); only a
    // wrapping flex is clamped. Decide at runtime when flex-wrap is dynamic.
    let new_preferred = match &flex.flex_wrap {
        Some(nr) => {
            let wrap_enum = crate::typeregister::BUILTIN.enums.FlexboxLayoutWrap.clone();
            let is_no_wrap = llr_Expression::BinaryExpression {
                lhs: Box::new(llr_Expression::PropertyReference(ctx.map_property_reference(nr))),
                rhs: Box::new(llr_Expression::EnumerationValue(EnumerationValue {
                    value: 1, // FlexboxLayoutWrap::NoWrap
                    enumeration: wrap_enum,
                })),
                op: '=',
            };
            llr_Expression::Condition {
                condition: Box::new(is_no_wrap),
                true_expr: Box::new(stored_field("preferred")),
                false_expr: Box::new(clamped),
            }
        }
        None => clamped, // default flex-wrap is `wrap`
    };

    let mut values =
        ty.fields.keys().map(|p| (p.clone(), stored_field(p))).collect::<BTreeMap<_, _>>();
    values.insert("preferred".into(), new_preferred);
    let clamped_struct = llr_Expression::Struct { ty: ty.clone(), values };

    // When the direction is known at compile time to be the main axis, clamp
    // unconditionally. When it is only known at runtime, clamp only in the
    // branch where the main axis is this cross axis and keep the computed
    // layout-info otherwise -- mirrors the runtime dispatch in
    // `compute_flexbox_layout_info` and the interpreter's runtime direction eval.
    let result = match axis_relation {
        FlexboxAxisRelation::MainAxis => clamped_struct,
        FlexboxAxisRelation::CrossAxis => unreachable!("returned early above"),
        FlexboxAxisRelation::Unknown => {
            let direction_enum = crate::typeregister::BUILTIN.enums.FlexboxLayoutDirection.clone();
            let direction_ref = llr_Expression::PropertyReference(
                ctx.map_property_reference(flex.direction.as_ref().unwrap()),
            );
            // The main axis is this cross axis when the direction is, for
            // Horizontal: Row (0) or RowReverse (1); for Vertical: Column (2) or
            // ColumnReverse (3).
            let (main_a, main_b) = match orientation {
                Orientation::Horizontal => (0, 1),
                Orientation::Vertical => (2, 3),
            };
            let is_direction = |value: usize| llr_Expression::BinaryExpression {
                lhs: Box::new(direction_ref.clone()),
                rhs: Box::new(llr_Expression::EnumerationValue(EnumerationValue {
                    value,
                    enumeration: direction_enum.clone(),
                })),
                op: '=',
            };
            let main_is_cross = llr_Expression::BinaryExpression {
                lhs: Box::new(is_direction(main_a)),
                rhs: Box::new(is_direction(main_b)),
                op: '|',
            };
            llr_Expression::Condition {
                condition: Box::new(main_is_cross),
                true_expr: Box::new(clamped_struct),
                false_expr: Box::new(stored()),
            }
        }
    };

    llr_Expression::CodeBlock([store, result].into())
}

struct GridLayoutCellConstraintsResult {
    cells: llr_Expression,
    /// When there are repeater involved, we need to do a WithLayoutItemInfo with the
    /// given cell variable and elements
    compute_cells: Option<(String, Vec<Either<llr_Expression, LayoutRepeatedElement>>)>,
}

/// Name of the local the generated GridLayout vertical pass binds to the index
/// of the repeated instance it is about to measure.
pub const GRID_MEASURE_REPEATER_INDEX_LOCAL: &str = "grid_measure_repeater_index";

/// Name of the local the generated repeated-Row `layout_item_info` binds to the
/// flattened index of the child it is about to measure.
pub const GRID_MEASURE_CHILD_INDEX_LOCAL: &str = "grid_measure_child_index";

/// Which slot of the cell's cache read the measuring loop fills in, and so
/// which local it binds. These are the only locals [`grid_measure_cross_width`]
/// introduces.
pub enum GridMeasureIndex {
    /// A repeated cell of the grid, addressed by its repeater index.
    Instance,
    /// A child of a repeated Row, addressed by its flattened index within the
    /// Row. That index is what the cache slot uses, so the expression built
    /// from one child serves every child that returns `Some` here — which is
    /// what `RowChildTemplateInfo::Repeated`'s `measure_at_cross_width` records.
    RowChild,
}

/// Reads a repeated grid cell's solved column width out of the grid's
/// horizontal cache: the cell's own `width` binding with `index`'s slot swapped
/// for a local, so the measuring loop can evaluate it once per instance.
///
/// `None` when the instance is not height-for-width (nothing to re-measure) or
/// when its width is fixed — the grid then never assigns it one, so there is no
/// cache binding to read.
pub fn grid_measure_cross_width(
    ctx: &mut ExpressionLoweringCtx,
    elem: &ElementRc,
    index: GridMeasureIndex,
) -> Option<llr_Expression> {
    let comp = elem.borrow().base_type.as_component().clone();
    let root = &comp.root_element;
    if !root.borrow().has_inherited_layout_info_v_with_constraint() {
        return None;
    }
    let mut width = repeated_cell_width_binding(root)?;
    let crate::expression_tree::Expression::GridRepeaterCacheAccess {
        repeater_index,
        inner_repeater_index,
        ..
    } = width.ignore_debug_hooks_mut()
    else {
        return None;
    };
    let local = |name: &str| crate::expression_tree::Expression::ReadLocalVariable {
        name: name.into(),
        ty: Type::Int32,
    };
    match index {
        GridMeasureIndex::Instance => **repeater_index = local(GRID_MEASURE_REPEATER_INDEX_LOCAL),
        GridMeasureIndex::RowChild => {
            *inner_repeater_index = Some(Box::new(local(GRID_MEASURE_CHILD_INDEX_LOCAL)))
        }
    }
    Some(super::lower_expression::lower_expression(&width, ctx))
}

/// The `width` binding a GridLayout gave a repeated cell, followed through
/// `geometry_props`: an injected wrapper (`Opacity`, `Transform`, …) becomes the
/// repeated component's root but leaves the binding on the element below it.
fn repeated_cell_width_binding(root: &ElementRc) -> Option<crate::expression_tree::Expression> {
    let width = root.borrow().geometry_props.as_ref()?.width.clone();
    let expr = width.element().borrow().binding(width.name())?.expression.clone();
    Some(expr)
}

fn grid_layout_cell_constraints(
    layout: &crate::layout::GridLayout,
    orientation: Orientation,
    ctx: &mut ExpressionLoweringCtx,
    cross_axis_size_override: Option<&crate::expression_tree::Expression>,
) -> GridLayoutCellConstraintsResult {
    let repeater_count =
        layout.elems.iter().filter(|i| i.item.element.borrow().repeated.is_some()).count();

    let element_ty = crate::typeregister::layout_item_info_type();

    if repeater_count == 0 {
        let cells = llr_Expression::Array {
            element_ty,
            values: layout
                .elems
                .iter()
                .map(|li| {
                    let layout_info = cell_layout_info(
                        &li.item.element,
                        &li.item.constraints,
                        ctx,
                        orientation,
                        cross_axis_size_override,
                        None,
                        false,
                    );
                    make_layout_cell_data_struct(layout_info, None, None)
                })
                .collect(),
            output: llr_ArrayOutput::Slice,
        };
        GridLayoutCellConstraintsResult { cells, compute_cells: None }
    } else {
        let mut elements = Vec::new();
        for item in &layout.elems {
            if item.item.element.borrow().repeated.is_some() {
                let repeater_index = match ctx
                    .mapping
                    .element_mapping
                    .get(&item.item.element.clone().into())
                    .unwrap()
                {
                    LoweredElement::Repeated { repeated_index } => *repeated_index,
                    _ => panic!(),
                };
                let row_child_templates = get_row_child_templates(&item.item.element, ctx);
                // Measure a height-for-width instance at the width the grid
                // assigns it, instead of its preferred width. Skipped when
                // reading the horizontal cache would close a binding loop
                // (`h_solve_reads_v_cache`), and on the
                // `layoutinfo-v-with-constraint` path, where the caller has
                // not settled the grid's width yet.
                let cross_width = (orientation == Orientation::Vertical
                    && cross_axis_size_override.is_none()
                    && row_child_templates.is_none()
                    && !item.cell.borrow().h_solve_reads_v_cache)
                    .then(|| {
                        grid_measure_cross_width(
                            ctx,
                            &item.item.element,
                            GridMeasureIndex::Instance,
                        )
                    })
                    .flatten();
                elements.push(Either::Right(LayoutRepeatedElement {
                    repeater_index,
                    row_child_templates,
                    cross_width,
                }));
            } else {
                let layout_info = cell_layout_info(
                    &item.item.element,
                    &item.item.constraints,
                    ctx,
                    orientation,
                    cross_axis_size_override,
                    None,
                    false,
                );
                elements.push(Either::Left(make_layout_cell_data_struct(layout_info, None, None)));
            }
        }
        let cells = llr_Expression::ReadLocalVariable {
            name: "cells".into(),
            ty: Type::Array(Arc::new(crate::typeregister::layout_info_type().into())),
        };
        GridLayoutCellConstraintsResult { cells, compute_cells: Some(("cells".into(), elements)) }
    }
}

struct GridLayoutInputDataResult {
    cells: llr_Expression,
    /// When there are repeaters involved, we need to do a WithGridInputData with the
    /// given cell variable and elements
    compute_cells: Option<(String, Vec<Either<llr_Expression, GridLayoutRepeatedElement>>)>,
}

// helper for organize_grid_layout()
fn grid_layout_input_data(
    layout: &crate::layout::GridLayout,
    ctx: &mut ExpressionLoweringCtx,
) -> GridLayoutInputDataResult {
    let propref = |named_ref: &RowColExpr| match named_ref {
        RowColExpr::Literal(n) => llr_Expression::NumberLiteral((*n).into()),
        RowColExpr::Named(nr) => llr_Expression::PropertyReference(ctx.map_property_reference(nr)),
        RowColExpr::Auto => llr_Expression::NumberLiteral(i_slint_common::ROW_COL_AUTO as _),
    };
    let input_data_for_cell = |elem: &crate::layout::GridLayoutElement,
                               new_row_expr: llr_Expression| {
        let row_expr = propref(&elem.cell.borrow().row_expr);
        let col_expr = propref(&elem.cell.borrow().col_expr);
        let rowspan_expr = propref(&elem.cell.borrow().rowspan_expr);
        let colspan_expr = propref(&elem.cell.borrow().colspan_expr);

        make_struct(
            BuiltinStruct::GridLayoutInputData,
            [
                ("new_row", Type::Bool, new_row_expr),
                ("row", Type::Float32, row_expr),
                ("col", Type::Float32, col_expr),
                ("rowspan", Type::Float32, rowspan_expr),
                ("colspan", Type::Float32, colspan_expr),
            ],
        )
    };
    let repeater_count =
        layout.elems.iter().filter(|i| i.item.element.borrow().repeated.is_some()).count();

    let element_ty = grid_layout_input_data_ty();

    if repeater_count == 0 {
        let cells = llr_Expression::Array {
            element_ty,
            values: layout
                .elems
                .iter()
                .map(|elem| {
                    input_data_for_cell(
                        elem,
                        llr_Expression::BoolLiteral(elem.cell.borrow().new_row),
                    )
                })
                .collect(),
            output: llr_ArrayOutput::Slice,
        };
        GridLayoutInputDataResult { cells, compute_cells: None }
    } else {
        let mut elements = Vec::new();
        let mut after_repeater_in_same_row = false;
        for item in &layout.elems {
            let new_row = item.cell.borrow().new_row;
            if new_row {
                after_repeater_in_same_row = false;
            }
            if item.item.element.borrow().repeated.is_some() {
                let repeater_index = match ctx
                    .mapping
                    .element_mapping
                    .get(&item.item.element.clone().into())
                    .unwrap()
                {
                    LoweredElement::Repeated { repeated_index } => *repeated_index,
                    _ => panic!(),
                };
                let row_child_templates = get_row_child_templates(&item.item.element, ctx);
                let repeated_element =
                    GridLayoutRepeatedElement { new_row, repeater_index, row_child_templates };
                elements.push(Either::Right(repeated_element));
                after_repeater_in_same_row = true;
            } else {
                let new_row_expr = if new_row || !after_repeater_in_same_row {
                    llr_Expression::BoolLiteral(new_row)
                } else {
                    llr_Expression::ReadLocalVariable {
                        name: SmolStr::new_static("new_row"),
                        ty: Type::Bool,
                    }
                };
                elements.push(Either::Left(input_data_for_cell(item, new_row_expr)));
            }
        }
        let cells = llr_Expression::ReadLocalVariable {
            name: "cells".into(),
            ty: Type::Array(Arc::new(element_ty)),
        };
        GridLayoutInputDataResult { cells, compute_cells: Some(("cells".into(), elements)) }
    }
}

pub(super) fn grid_layout_input_data_ty() -> Type {
    Type::Struct(Arc::new(Struct::new(
        IntoIterator::into_iter([
            (SmolStr::new_static("new_row"), Type::Bool),
            (SmolStr::new_static("row"), Type::Int32),
            (SmolStr::new_static("col"), Type::Int32),
            (SmolStr::new_static("rowspan"), Type::Int32),
            (SmolStr::new_static("colspan"), Type::Int32),
        ])
        .collect(),
        BuiltinStruct::GridLayoutInputData,
    )))
}

fn generate_layout_padding_and_spacing(
    layout_geometry: &crate::layout::LayoutGeometry,
    orientation: Orientation,
    ctx: &ExpressionLoweringCtx,
) -> (llr_Expression, llr_Expression) {
    let padding_prop = |expr| {
        if let Some(expr) = expr {
            llr_Expression::PropertyReference(ctx.map_property_reference(expr))
        } else {
            llr_Expression::NumberLiteral(0.)
        }
    };
    let spacing = padding_prop(layout_geometry.spacing.orientation(orientation));
    let (begin, end) = layout_geometry.padding.begin_end(orientation);

    let padding = make_struct(
        BuiltinStruct::Padding,
        [("begin", Type::Float32, padding_prop(begin)), ("end", Type::Float32, padding_prop(end))],
    );

    (padding, spacing)
}

/// Whether `elem` is a height-for-width cell — its vertical layout info
/// depends on the horizontal dimension, so a cross-axis constraint must
/// be supplied to get a meaningful answer.
///
/// Two cases qualify:
/// - Builtin height-for-width items (Text with `wrap != no-wrap`, Image with
///   aspect-ratio sizing).
/// - Components whose subtree contains a height-for-width descendant — recognized
///   by the presence of `Element::layout_info_v_with_constraint`.
fn is_height_for_width_cell(elem: &ElementRc) -> bool {
    let elem_b = elem.borrow();

    // Component path: `layoutinfo-v-with-constraint` may live on `elem`
    // itself or on the base component's root_element.
    let has_constrained_layoutinfo_v = elem_b.layout_info_v_with_constraint.is_some()
        || matches!(
            &elem_b.base_type,
            crate::langtype::ElementType::Component(base_comp)
                if base_comp.root_element.borrow().layout_info_v_with_constraint.is_some()
        );
    if has_constrained_layoutinfo_v {
        return true;
    }

    if elem_b.layout_info_prop(Orientation::Vertical).is_some() {
        return false;
    }
    drop(elem_b);

    // Builtin path.
    matches!(
        crate::layout::implicit_layout_info_call(
            elem,
            Orientation::Vertical,
            crate::layout::BuiltinFilter::All,
            None,
        ),
        Some(crate::expression_tree::Expression::FunctionCall { .. })
    )
}

/// Default cross-axis (width) constraint for a height-for-width cell:
/// the element's own preferred horizontal size. Callers
/// (`flexbox_layout_data`, `box_layout_data`,
/// `grid_layout_cell_constraints`) may prefer the container's actual
/// width when it is available (i.e. at solve time, or when the caller
/// is the body of a `layoutinfo-v-with-constraint` function which
/// received the width as a parameter).
///
/// Precondition: `is_height_for_width_cell(elem)` is true. After the
/// `layoutinfo-v-with-constraint` synthesis pass, any element with
/// `layout_info_v_with_constraint` also has `layout_info_prop` set (the
/// constrained function is synthesized from the existing `layoutinfo-v`
/// binding), so the `layout_info_prop` branch covers it.
pub(crate) fn default_cross_axis_constraint(
    elem: &ElementRc,
) -> Option<crate::expression_tree::Expression> {
    let elem_b = elem.borrow();

    // Route through `layoutinfo-h-with-constraint` when available so we
    // don't trigger a `self.height` read (which cycles for column-direction
    // flexes: their layoutinfo-h depends on self.height, itself set by the
    // parent layout cache). The NR returned by `inherited_*` already points
    // to the element declaring the function (which, after
    // `move_declarations` runs, is the enclosing component's root with a
    // renamed property), so use it as-is — re-anchoring it to `elem` would
    // break the lookup.
    if let Some(constrained_nr) = elem_b.inherited_layout_info_h_with_constraint() {
        let call = crate::expression_tree::Expression::FunctionCall {
            function: crate::expression_tree::Callable::Function(constrained_nr),
            arguments: vec![crate::expression_tree::Expression::NumberLiteral(
                f32::MAX as f64,
                crate::expression_tree::Unit::Px,
            )],
            source_location: None,
        };
        return Some(crate::expression_tree::Expression::StructFieldAccess {
            base: Box::new(call),
            name: "preferred".into(),
        });
    }

    // Layouts and components with their own resolved layout_info_prop.
    if let Some((h_nr, _v_nr)) = elem_b.layout_info_prop.as_ref() {
        return Some(crate::expression_tree::Expression::StructFieldAccess {
            base: Box::new(crate::expression_tree::Expression::PropertyReference(h_nr.clone())),
            name: "preferred".into(),
        });
    }
    drop(elem_b);

    // Builtins and component instances (looked up via the base component).
    crate::layout::implicit_layout_info_call(
        elem,
        Orientation::Horizontal,
        crate::layout::BuiltinFilter::All,
        None,
    )
    .map(|expr| crate::expression_tree::Expression::StructFieldAccess {
        base: Box::new(expr),
        name: "preferred".into(),
    })
}

/// Subtract `geometry`'s padding on the `axis` from `base`. Turns an outer size
/// into the content size a child is actually laid out at (used to constrain a
/// height-for-width child at its real width rather than the padded outer width).
fn subtract_padding(
    base: crate::expression_tree::Expression,
    geometry: &crate::layout::LayoutGeometry,
    axis: Orientation,
) -> crate::expression_tree::Expression {
    use crate::expression_tree::Expression;
    let pads = match axis {
        Orientation::Horizontal => [&geometry.padding.left, &geometry.padding.right],
        Orientation::Vertical => [&geometry.padding.top, &geometry.padding.bottom],
    };
    let mut expr = base;
    for p in pads.into_iter().flatten() {
        expr = Expression::BinaryExpression {
            lhs: Box::new(expr),
            rhs: Box::new(Expression::PropertyReference(p.clone())),
            op: '-',
        };
    }
    expr
}

/// Build an expression for the layout's cross-axis *content* size
/// (`self.height` minus top/bottom padding, for a horizontal layout).
fn layout_cross_content_size(
    layout: &crate::layout::BoxLayout,
) -> Option<crate::expression_tree::Expression> {
    use crate::expression_tree::Expression;
    let cross = layout.orientation.orthogonal();
    let size_nr = layout.geometry.rect.size_reference(cross)?.clone();
    Some(subtract_padding(Expression::PropertyReference(size_nr), &layout.geometry, cross))
}

fn layout_geometry_size(
    rect: &crate::layout::LayoutRect,
    orientation: Orientation,
    ctx: &ExpressionLoweringCtx,
) -> llr_Expression {
    match rect.size_reference(orientation) {
        Some(nr) => llr_Expression::PropertyReference(ctx.map_property_reference(nr)),
        None => llr_Expression::NumberLiteral(0.),
    }
}

/// A flex cell's `LayoutInfo`: same as [`get_layout_info`] but keeps only the
/// cell's *locally-set* constraints on top of the measured layout-info. An
/// inherited intrinsic min/max/preferred is already included in `layout_info`
/// (via the parametrized `layoutinfo-{h,v}-with-constraint`), and re-reading it
/// unconstrained would reintroduce a height-for-width loop through the flex
/// solve. Only flex callers need this: box/grid measure the cell without the
/// cross-axis being under solve, so re-applying inherited constraints there
/// doesn't cycle — and it's necessary, because those inherited constraints
/// aren't merged into the layout-info of a cell with a fixed size binding
/// (see `default_geometry::gen_layout_info_prop`).
pub fn get_flex_cell_layout_info(
    elem: &ElementRc,
    ctx: &mut ExpressionLoweringCtx,
    constraints: &crate::layout::LayoutConstraints,
    orientation: Orientation,
    constraint: Option<crate::expression_tree::Expression>,
) -> llr_Expression {
    let effective = constraints.to_apply(elem, orientation);
    get_layout_info(elem, ctx, &effective, orientation, constraint)
}

pub fn get_layout_info(
    elem: &ElementRc,
    ctx: &mut ExpressionLoweringCtx,
    constraints: &crate::layout::LayoutConstraints,
    orientation: Orientation,
    constraint: Option<crate::expression_tree::Expression>,
) -> llr_Expression {
    // With a constraint and a parameterized layout-info function on the
    // child, call that function instead of reading the plain
    // `layoutinfo-{h,v}` property — breaks the recursion via the child's
    // perpendicular dimension.
    let layout_info = if let Some(c) = &constraint
        && let Some(parameterized_nr) = (match orientation {
            Orientation::Vertical => elem.borrow().layout_info_v_with_constraint.clone(),
            Orientation::Horizontal => elem.borrow().layout_info_h_with_constraint.clone(),
        }) {
        let call = crate::expression_tree::Expression::FunctionCall {
            function: crate::expression_tree::Callable::Function(parameterized_nr),
            arguments: vec![c.clone()],
            source_location: None,
        };
        super::lower_expression::lower_expression(&call, ctx)
    } else if let Some(layout_info_prop) = &elem.borrow().layout_info_prop(orientation) {
        llr_Expression::PropertyReference(ctx.map_property_reference(layout_info_prop))
    } else {
        super::lower_expression::lower_expression(
            &crate::layout::implicit_layout_info_call(
                elem,
                orientation,
                crate::layout::BuiltinFilter::All,
                constraint,
            )
            .unwrap(),
            ctx,
        )
    };

    if constraints.has_explicit_restrictions(orientation) {
        let store = llr_Expression::StoreLocalVariable {
            name: "layout_info".into(),
            value: layout_info.into(),
        };
        let ty = crate::typeregister::layout_info_type();
        let mut values = ty
            .fields
            .keys()
            .map(|p| {
                (
                    p.clone(),
                    llr_Expression::StructFieldAccess {
                        base: llr_Expression::ReadLocalVariable {
                            name: "layout_info".into(),
                            ty: ty.clone().into(),
                        }
                        .into(),
                        name: p.clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        for (nr, s) in constraints.for_each_restrictions(orientation) {
            values.insert(
                s.into(),
                llr_Expression::PropertyReference(ctx.map_property_reference(nr)),
            );
        }
        llr_Expression::CodeBlock([store, llr_Expression::Struct { ty, values }].into())
    } else {
        layout_info
    }
}

// Called for repeated components in a grid layout, to generate code to provide input for organize_grid_layout().
pub fn get_grid_layout_input_for_repeated(
    ctx: &mut ExpressionLoweringCtx,
    grid_cell: &GridLayoutCell,
) -> llr_Expression {
    let mut assignments = Vec::new();

    fn convert_row_col_expr(expr: &RowColExpr, ctx: &ExpressionLoweringCtx) -> llr_Expression {
        match expr {
            RowColExpr::Literal(n) => llr_Expression::NumberLiteral((*n).into()),
            RowColExpr::Named(nr) => {
                llr_Expression::PropertyReference(ctx.map_property_reference(nr))
            }
            RowColExpr::Auto => llr_Expression::NumberLiteral(i_slint_common::ROW_COL_AUTO as _),
        }
    }

    // Generate assignments to the `result` slice parameter: result[i] = struct { ... }
    let mut push_assignment =
        |i: usize, new_row_expr: &llr_Expression, grid_cell: &GridLayoutCell| {
            let row = convert_row_col_expr(&grid_cell.row_expr, &*ctx);
            let col = convert_row_col_expr(&grid_cell.col_expr, &*ctx);
            let rowspan = convert_row_col_expr(&grid_cell.rowspan_expr, &*ctx);
            let colspan = convert_row_col_expr(&grid_cell.colspan_expr, &*ctx);
            let value = make_struct(
                BuiltinStruct::GridLayoutInputData,
                [
                    ("new_row", Type::Bool, new_row_expr.clone()),
                    ("row", Type::Float32, row),
                    ("col", Type::Float32, col),
                    ("rowspan", Type::Float32, rowspan),
                    ("colspan", Type::Float32, colspan),
                ],
            );
            assignments.push(llr_Expression::SliceIndexAssignment {
                slice_name: SmolStr::new_static("result"),
                index: i,
                value: value.into(),
            });
        };

    if let Some(child_items) = grid_cell.child_items.as_ref() {
        // Repeated Row: only handle static children here;
        // inner repeater children are handled by the code generators at runtime
        let mut new_row_expr = llr_Expression::BoolLiteral(true);
        let mut i = 0;
        for child_item in child_items.iter() {
            match child_item {
                crate::layout::RowChildTemplate::Static(layout_item) => {
                    let child_element = layout_item.element.borrow();
                    let child_cell = child_element.grid_layout_cell.as_ref().unwrap().borrow();
                    push_assignment(i, &new_row_expr, &child_cell);
                    new_row_expr = llr_Expression::BoolLiteral(false);
                    i += 1;
                }
                crate::layout::RowChildTemplate::Repeated { .. } => {
                    // Inner repeater children are filled at runtime by the code generators
                }
            }
        }
    } else {
        // Single repeated item
        // grid_cell.new_row is the static information from the slint file.
        // In practice, for repeated items within a row, whether we should start a new row
        // is more dynamic (e.g. if the previous item was in "if false"),
        // and tracked by a local variable "new_row" in the generated code.
        let new_row_expr = llr_Expression::ReadLocalVariable {
            name: SmolStr::new_static("new_row"),
            ty: Type::Bool,
        };
        push_assignment(0, &new_row_expr, grid_cell);
    }

    llr_Expression::CodeBlock(assignments)
}

/// Returns the row child template list for a repeated Row element.
///
/// Reads it from the already-lowered Row sub-component (which must have been
/// lowered before the parent's expression lowering — see the ordering in
/// `lower_sub_component`).
///
/// Returns `None` if this is a column-repeater (not a Row sub-component).
/// Returns `Some(vec)` with one entry per child in declaration order.
fn get_row_child_templates(
    outer_element: &ElementRc,
    ctx: &ExpressionLoweringCtx,
) -> Option<Vec<super::RowChildTemplateInfo>> {
    let comp = outer_element.borrow().base_type.as_component().clone();
    ctx.state.row_child_templates(&comp)
}

/// Generate an expression that builds a FlexboxLayoutItemInfo for a repeated element
/// in a FlexboxLayout, reading flex properties from the component instance.
pub fn get_flexbox_layout_item_info_for_repeated(
    ctx: &mut ExpressionLoweringCtx,
    element: &ElementRc,
) -> llr_Expression {
    let prop_ref = |name: &'static str| -> Option<llr_Expression> {
        crate::layout::binding_reference(element, name)
            .map(|nr| llr_Expression::PropertyReference(ctx.map_property_reference(&nr)))
    };

    let (_, align_self_default) = default_align_self();

    let align_self = prop_ref("cross-axis-self-alignment").unwrap_or(align_self_default);
    let order = prop_ref("layout-order").unwrap_or(llr_Expression::NumberLiteral(0.0));

    make_struct(
        BuiltinStruct::FlexboxLayoutItemInfo,
        [
            (
                "constraint",
                crate::typeregister::layout_info_type().into(),
                llr_Expression::default_value_for_type(
                    &crate::typeregister::layout_info_type().into(),
                )
                .unwrap(),
            ),
            (
                "props",
                crate::typeregister::flex_item_props_type(),
                make_flex_props_struct(FlexItemProps { align_self, order }),
            ),
        ],
    )
}

/// Vertical `LayoutInfo` for a repeated element, computed with the element's
/// preferred width as the cross-axis constraint. Routes through the element's
/// `layoutinfo-v-with-constraint` (via [`get_layout_info`]), so a
/// height-for-width instance in a column FlexboxLayout computes its height from
/// that width instead of reading `self.width` — which would cycle through the
/// parent flex's layout cache. Returns `None` when the element has no
/// constrained vertical layout-info (nothing to break).
pub fn get_layout_info_v_constrained_for_repeated(
    ctx: &mut ExpressionLoweringCtx,
    element: &ElementRc,
    constraints: &crate::layout::LayoutConstraints,
) -> Option<llr_Expression> {
    if !element.borrow().has_inherited_layout_info_v_with_constraint() {
        return None;
    }
    // Use the preferred width as the cross-axis constraint, the same default
    // static height-for-width cells use. This is a single-line-height
    // approximation; a column flex re-measures at the real container width via
    // `get_layout_info_v_at_cross_width_for_repeated`.
    //
    // The h-constraint may be absent even when the v one exists; fall back to
    // unbounded then.
    let width_constraint = default_cross_axis_constraint(element).unwrap_or_else(|| {
        crate::expression_tree::Expression::NumberLiteral(
            f32::MAX as f64,
            crate::expression_tree::Unit::Px,
        )
    });
    Some(get_flex_cell_layout_info(
        element,
        ctx,
        constraints,
        Orientation::Vertical,
        Some(width_constraint),
    ))
}

/// Name of the local that carries the cross-axis (container) width into the
/// generated `flexbox_layout_item_info_at_cross_width` and
/// `layout_item_info_at_cross_width` method bodies (the parameter name is
/// derived from it).
pub const CROSS_WIDTH_LOCAL: &str = "cross_width";

/// Like [`get_layout_info_v_constrained_for_repeated`], but measures at the
/// width passed in the [`CROSS_WIDTH_LOCAL`] local instead of the
/// element's preferred width. A column FlexboxLayout (or a box layout)
/// supplies the width it assigns the instance here at solve time, so a
/// repeated height-for-width instance gets the same wrapped height as an
/// equivalent static cell. Returns `None` when the element has no constrained
/// vertical layout-info.
///
/// `for_flex_cell` selects [`get_flex_cell_layout_info`] (flexbox:
/// re-reading inherited constraints unconstrained would reintroduce the
/// height-for-width cycle); every other layout kind uses [`get_layout_info`]
/// (inherited constraints are re-applied, like for static box cells).
pub fn get_layout_info_v_at_cross_width_for_repeated(
    ctx: &mut ExpressionLoweringCtx,
    element: &ElementRc,
    constraints: &crate::layout::LayoutConstraints,
    for_flex_cell: bool,
) -> Option<llr_Expression> {
    if !element.borrow().has_inherited_layout_info_v_with_constraint() {
        return None;
    }
    let width_constraint = crate::expression_tree::Expression::ReadLocalVariable {
        name: CROSS_WIDTH_LOCAL.into(),
        ty: Type::LogicalLength,
    };
    let get = if for_flex_cell { get_flex_cell_layout_info } else { get_layout_info };
    Some(get(element, ctx, constraints, Orientation::Vertical, Some(width_constraint)))
}

/// Horizontal `LayoutInfo` for a repeated element, computed with an unbounded
/// height constraint. Routes through the element's
/// `layoutinfo-h-with-constraint` (via [`get_layout_info`]), so a
/// width-for-height instance in a FlexboxLayout computes its width from that
/// height instead of reading `self.height` — which would cycle through the
/// parent flex's layout cache. Returns `None` when the element has no
/// constrained horizontal layout-info (nothing to break).
pub fn get_layout_info_h_constrained_for_repeated(
    ctx: &mut ExpressionLoweringCtx,
    element: &ElementRc,
    constraints: &crate::layout::LayoutConstraints,
) -> Option<llr_Expression> {
    element.borrow().inherited_layout_info_h_with_constraint()?;
    // Unbounded, the same default static width-for-height cells use: the
    // natural, unwrapped width. A flex re-measures at the height it really
    // assigns via `get_layout_info_h_at_cross_height_for_repeated`.
    let height_constraint = crate::expression_tree::Expression::NumberLiteral(
        f32::MAX as f64,
        crate::expression_tree::Unit::Px,
    );
    Some(get_flex_cell_layout_info(
        element,
        ctx,
        constraints,
        Orientation::Horizontal,
        Some(height_constraint),
    ))
}

/// Name of the local that carries the cross-axis (assigned) height into the
/// generated `flexbox_layout_item_info_at_cross_height` and
/// `layout_item_info_at_cross_height` method bodies (the parameter name is
/// derived from it).
pub const CROSS_HEIGHT_LOCAL: &str = "cross_height";

/// Like [`get_layout_info_h_constrained_for_repeated`], but measures at the
/// height passed in the [`CROSS_HEIGHT_LOCAL`] local instead of leaving it
/// unbounded. A FlexboxLayout (or a box layout) supplies the height it
/// assigned at solve time, so a repeated width-for-height instance gets the
/// same width as an equivalent static cell. Returns `None` when the element
/// has no constrained horizontal layout-info. See
/// [`get_layout_info_v_at_cross_width_for_repeated`] for `for_flex_cell`.
pub fn get_layout_info_h_at_cross_height_for_repeated(
    ctx: &mut ExpressionLoweringCtx,
    element: &ElementRc,
    constraints: &crate::layout::LayoutConstraints,
    for_flex_cell: bool,
) -> Option<llr_Expression> {
    element.borrow().inherited_layout_info_h_with_constraint()?;
    let height_constraint = crate::expression_tree::Expression::ReadLocalVariable {
        name: CROSS_HEIGHT_LOCAL.into(),
        ty: Type::LogicalLength,
    };
    let get = if for_flex_cell { get_flex_cell_layout_info } else { get_layout_info };
    Some(get(element, ctx, constraints, Orientation::Horizontal, Some(height_constraint)))
}
