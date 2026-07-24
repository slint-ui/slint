// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::BTreeMap;
use std::rc::Rc;

use itertools::Either;
use smol_str::SmolStr;

use super::lower_to_item_tree::LoweredElement;
use super::{GridLayoutRepeatedElement, LayoutRepeatedElement};
use crate::expression_tree::MinMaxOp;
use crate::langtype::{BuiltinStruct, EnumerationValue, Struct, Type};
use crate::layout::{FlexboxAxisRelation, GridLayoutCell, Orientation, RowColExpr};
use crate::llr::ArrayOutput as llr_ArrayOutput;
use crate::llr::Expression as llr_Expression;
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
        enumeration: crate::typeregister::BUILTIN.with(|b| b.enums.Orientation.clone()),
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
            sub_expression: Box::new(sub_expression),
        },
        None => sub_expression,
    }
}

pub(super) fn compute_box_layout_info(
    layout: &crate::layout::BoxLayout,
    o: Orientation,
    ctx: &mut ExpressionLoweringCtx,
    cross_axis_size_override: Option<&crate::expression_tree::Expression>,
) -> llr_Expression {
    let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
    let adjusted_override = cross_axis_size_override
        .map(|o_expr| subtract_padding(o_expr.clone(), &layout.geometry, o.orthogonal()));
    let bld = box_layout_data(layout, o, ctx, adjusted_override.as_ref(), None);
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
    match bld.compute_cells {
        Some((cells_variable, elements)) => llr_Expression::WithLayoutItemInfo {
            cells_variable,
            repeater_indices_var_name: None,
            repeater_steps_var_name: None,
            elements,
            orientation: o,
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
        let e = crate::typeregister::BUILTIN.with(|e| e.enums.DialogButtonRole.clone());
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
        enumeration: crate::typeregister::BUILTIN.with(|b| b.enums.Orientation.clone()),
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
    // child matches the height it will actually be given.
    let cross_override = (o == layout.orientation && o == Orientation::Horizontal)
        .then(|| layout_cross_content_size(layout))
        .flatten();
    // On the cross pass, the layout's content size along `o` is its
    // cross content size; forward it so a wrapping perpendicular flex cell
    // gets its natural single-line size instead of the compact sqrt preferred.
    let cross_clamp =
        (o != layout.orientation).then(|| layout_cross_content_size(layout)).flatten();
    let bld = box_layout_data(layout, o, ctx, cross_override.as_ref(), cross_clamp.as_ref());
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
                    crate::typeregister::BUILTIN
                        .with(|e| Type::Enumeration(e.enums.LayoutAlignment.clone())),
                    bld.alignment,
                ),
                ("cells", bld.cells.ty(ctx), bld.cells),
            ],
        );
        (data, "solve_box_layout")
    } else {
        let cross_axis_alignment_ty = crate::typeregister::BUILTIN
            .with(|e| Type::Enumeration(e.enums.CrossAxisAlignment.clone()));
        let cross_axis_alignment = if let Some(nr) = &layout.cross_alignment {
            llr_Expression::PropertyReference(ctx.map_property_reference(nr))
        } else {
            let e = crate::typeregister::BUILTIN.with(|e| e.enums.CrossAxisAlignment.clone());
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
                crate::typeregister::BUILTIN
                    .with(|e| Type::Enumeration(e.enums.LayoutAlignment.clone())),
                fld.alignment,
            ),
            (
                "direction",
                crate::typeregister::BUILTIN
                    .with(|e| Type::Enumeration(e.enums.FlexboxLayoutDirection.clone())),
                fld.direction,
            ),
            (
                "align_content",
                crate::typeregister::BUILTIN
                    .with(|e| Type::Enumeration(e.enums.FlexboxLayoutAlignContent.clone())),
                fld.align_content,
            ),
            (
                "cross_axis_alignment",
                crate::typeregister::BUILTIN
                    .with(|e| Type::Enumeration(e.enums.CrossAxisAlignment.clone())),
                fld.cross_axis_alignment,
            ),
            (
                "flex_wrap",
                crate::typeregister::BUILTIN
                    .with(|e| Type::Enumeration(e.enums.FlexboxLayoutWrap.clone())),
                fld.flex_wrap,
            ),
            ("cells_h", fld.cells_h.ty(ctx), fld.cells_h),
            ("cells_v", fld.cells_v.ty(ctx), fld.cells_v),
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
    let needs_measure = layout.elems.iter().any(|li| {
        let elem = &li.item.element;
        is_height_for_width_cell(elem)
            || elem.borrow().inherited_layout_info_h_with_constraint().is_some()
    });
    match fld.compute_cells {
        Some((cells_h_var, cells_v_var, elements)) => {
            let repeated_indices = || llr_Expression::ReadLocalVariable {
                name: "repeated_indices".into(),
                ty: Type::Array(Type::Int32.into()),
            };
            // With a repeater, the cell defaults come from the flat cell arrays
            // the enclosing `WithFlexboxLayoutItemInfo` just built, so no
            // `default_cells`.
            let sub_expression = if needs_measure {
                llr_Expression::SolveFlexboxLayoutWithMeasure {
                    data: Box::new(data),
                    repeater_indices: Box::new(repeated_indices()),
                    measure_cells: measure_cells_for(layout, ctx),
                    default_cells: vec![],
                    cells_variables: Some((cells_h_var.clone().into(), cells_v_var.clone().into())),
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
            // Preferred (default-constraint) info per cell, matching the cells
            // carried by `data`. Returned by the measure callback when taffy
            // asks for a dimension without a known cross-axis size, so the
            // both-unknown case mirrors the plain `solve_flexbox_layout`.
            let default_cells = layout
                .elems
                .iter()
                .map(|li| {
                    let elem = &li.item.element;
                    let v_constraint = if is_height_for_width_cell(elem) {
                        default_cross_axis_constraint(elem)
                    } else {
                        None
                    };
                    let v_info = get_layout_info(
                        elem,
                        ctx,
                        &li.item.constraints,
                        Orientation::Vertical,
                        v_constraint,
                    );
                    let h_constraint =
                        elem.borrow().inherited_layout_info_h_with_constraint().is_some().then(
                            || {
                                crate::expression_tree::Expression::NumberLiteral(
                                    f32::MAX as f64,
                                    crate::expression_tree::Unit::Px,
                                )
                            },
                        );
                    let h_info = get_layout_info(
                        elem,
                        ctx,
                        &li.item.constraints,
                        Orientation::Horizontal,
                        h_constraint,
                    );
                    Either::Left((h_info, v_info))
                })
                .collect();
            llr_Expression::SolveFlexboxLayoutWithMeasure {
                data: Box::new(data),
                repeater_indices: Box::new(empty_int32_slice()),
                measure_cells: measure_cells_for(layout, ctx),
                default_cells,
                cells_variables: None,
            }
        }
    }
}

/// Per-element measure inputs for `SolveFlexboxLayoutWithMeasure`: the cell's
/// `(h_info, v_info)` measured at the dimension taffy assigns, read from the
/// `measure_known_w` / `measure_known_h` locals. A repeated element becomes a
/// `Right`: its instances are only known at solve time, so the generated
/// callback queries the instance directly.
fn measure_cells_for(
    layout: &crate::layout::FlexboxLayout,
    ctx: &mut ExpressionLoweringCtx,
) -> Vec<Either<(llr_Expression, llr_Expression), LayoutRepeatedElement>> {
    layout
        .elems
        .iter()
        .map(|li| {
            let elem = &li.item.element;
            if elem.borrow().repeated.is_some() {
                let repeater_index =
                    match ctx.mapping.element_mapping.get(&elem.clone().into()).unwrap() {
                        LoweredElement::Repeated { repeated_index } => *repeated_index,
                        _ => panic!("repeated flexbox element not lowered as Repeated"),
                    };
                return Either::Right(LayoutRepeatedElement {
                    repeater_index,
                    row_child_templates: None,
                });
            }
            let v_constraint = is_height_for_width_cell(elem).then(|| {
                crate::expression_tree::Expression::ReadLocalVariable {
                    name: "measure_known_w".into(),
                    ty: Type::LogicalLength,
                }
            });
            let v_info = get_layout_info(
                elem,
                ctx,
                &li.item.constraints,
                Orientation::Vertical,
                v_constraint,
            );
            let h_constraint =
                elem.borrow().inherited_layout_info_h_with_constraint().is_some().then(|| {
                    crate::expression_tree::Expression::ReadLocalVariable {
                        name: "measure_known_h".into(),
                        ty: Type::LogicalLength,
                    }
                });
            let h_info = get_layout_info(
                elem,
                ctx,
                &li.item.constraints,
                Orientation::Horizontal,
                h_constraint,
            );
            Either::Left((h_info, v_info))
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
            let direction_enum =
                crate::typeregister::BUILTIN.with(|e| e.enums.FlexboxLayoutDirection.clone());
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
            spacing_h,
            spacing_v,
            padding_h,
            padding_v,
            fld.direction,
            fld.flex_wrap,
            constraint_size,
        ];

        match fld.compute_cells {
            Some((cells_h_var, cells_v_var, elements)) => {
                llr_Expression::WithFlexboxLayoutItemInfo {
                    cells_h_variable: cells_h_var,
                    cells_v_variable: cells_v_var,
                    repeater_indices_var_name: None,
                    elements,
                    // Info computation, not a solve: no container width to forward.
                    repeated_cross_width: None,
                    sub_expression: Box::new(llr_Expression::ExtraBuiltinFunctionCall {
                        function: "flexbox_layout_info_cross_axis".into(),
                        arguments,
                        return_ty: crate::typeregister::layout_info_type().into(),
                    }),
                }
            }
            None => llr_Expression::ExtraBuiltinFunctionCall {
                function: "flexbox_layout_info_cross_axis".into(),
                arguments,
                return_ty: crate::typeregister::layout_info_type().into(),
            },
        }
    } else {
        // Main axis: only needs same-axis cells, avoiding cross-axis binding loop.
        let (cells, spacing, padding) = match orientation {
            Orientation::Horizontal => (fld.cells_h, spacing_h, padding_h),
            Orientation::Vertical => (fld.cells_v, spacing_v, padding_v),
        };

        match fld.compute_cells {
            Some((cells_h_var, cells_v_var, elements)) => {
                let cells_var = match orientation {
                    Orientation::Horizontal => cells_h_var.clone(),
                    Orientation::Vertical => cells_v_var.clone(),
                };
                llr_Expression::WithFlexboxLayoutItemInfo {
                    cells_h_variable: cells_h_var,
                    cells_v_variable: cells_v_var,
                    repeater_indices_var_name: None,
                    elements,
                    // Info computation, not a solve: no container width to forward.
                    repeated_cross_width: None,
                    sub_expression: Box::new(llr_Expression::ExtraBuiltinFunctionCall {
                        function: "flexbox_layout_info_main_axis".into(),
                        arguments: vec![
                            llr_Expression::ReadLocalVariable {
                                name: cells_var.into(),
                                ty: Type::Array(Rc::new(
                                    crate::typeregister::flexbox_layout_item_info_type(),
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
    align_content: llr_Expression,
    cross_axis_alignment: llr_Expression,
    flex_wrap: llr_Expression,
    cells_h: llr_Expression,
    cells_v: llr_Expression,
    /// When there are repeaters involved, we need to do a WithFlexboxLayoutItemInfo with the
    /// given cells_h/cells_v variable names and elements (each static element has a tuple of (h, v) layout info)
    compute_cells: Option<(
        String,
        String,
        Vec<Either<(llr_Expression, llr_Expression), LayoutRepeatedElement>>,
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
        let e = crate::typeregister::BUILTIN.with(|e| e.enums.LayoutAlignment.clone());
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let direction = if let Some(expr) = &layout.direction {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.with(|e| e.enums.FlexboxLayoutDirection.clone());
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let align_content = if let Some(expr) = &layout.align_content {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.with(|e| e.enums.FlexboxLayoutAlignContent.clone());
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let cross_axis_alignment = if let Some(expr) = &layout.cross_axis_alignment {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.with(|e| e.enums.CrossAxisAlignment.clone());
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let flex_wrap = if let Some(expr) = &layout.flex_wrap {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.with(|e| e.enums.FlexboxLayoutWrap.clone());
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let repeater_count =
        layout.elems.iter().filter(|i| i.item.element.borrow().repeated.is_some()).count();

    let element_ty = crate::typeregister::flexbox_layout_item_info_type();

    let flex_prop =
        |li: &crate::layout::FlexboxLayoutItem, ctx: &mut ExpressionLoweringCtx| -> FlexItemProps {
            FlexItemProps {
                grow: li
                    .flex_grow
                    .as_ref()
                    .map(|nr| llr_Expression::PropertyReference(ctx.map_property_reference(nr)))
                    .unwrap_or(llr_Expression::NumberLiteral(0.0)),
                shrink: li
                    .flex_shrink
                    .as_ref()
                    .map(|nr| llr_Expression::PropertyReference(ctx.map_property_reference(nr)))
                    // CSS default, same as the interpreter and the repeated-cell path
                    .unwrap_or(llr_Expression::NumberLiteral(1.0)),
                basis: li
                    .flex_basis
                    .as_ref()
                    .map(|nr| llr_Expression::PropertyReference(ctx.map_property_reference(nr)))
                    .unwrap_or(llr_Expression::NumberLiteral(-1.0)),
                align_self: li
                    .align_self
                    .as_ref()
                    .map(|nr| llr_Expression::PropertyReference(ctx.map_property_reference(nr)))
                    .unwrap_or(default_align_self().1),
                order: li
                    .order
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
                    let constraint = cell_h_constraint(&li.item.element);
                    let layout_info_h = get_layout_info(
                        &li.item.element,
                        ctx,
                        &li.item.constraints,
                        Orientation::Horizontal,
                        constraint,
                    );
                    let flex_props = flex_prop(li, ctx);
                    make_flexbox_cell_data_struct(layout_info_h, flex_props)
                })
                .collect(),
            element_ty: element_ty.clone(),
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
                    let constraint = cell_v_constraint(&li.item.element);
                    let layout_info_v = get_layout_info(
                        &li.item.element,
                        ctx,
                        &li.item.constraints,
                        Orientation::Vertical,
                        constraint,
                    );
                    let flex_props = flex_prop(li, ctx);
                    make_flexbox_cell_data_struct(layout_info_v, flex_props)
                })
                .collect(),
            element_ty,
            output: llr_ArrayOutput::Slice,
        };
        FlexboxLayoutDataResult {
            alignment,
            direction,
            align_content,
            cross_axis_alignment,
            flex_wrap,
            cells_h,
            cells_v,
            compute_cells: None,
        }
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
                elements.push(Either::Right(LayoutRepeatedElement {
                    repeater_index,
                    row_child_templates: None,
                }))
            } else {
                // For static elements, we need both orientations
                let h_constraint = cell_h_constraint(&item.item.element);
                let layout_info_h = get_layout_info(
                    &item.item.element,
                    ctx,
                    &item.item.constraints,
                    Orientation::Horizontal,
                    h_constraint,
                );
                let constraint = cell_v_constraint(&item.item.element);
                let layout_info_v = get_layout_info(
                    &item.item.element,
                    ctx,
                    &item.item.constraints,
                    Orientation::Vertical,
                    constraint,
                );
                let flex_props = flex_prop(item, ctx);
                elements.push(Either::Left((
                    make_flexbox_cell_data_struct(layout_info_h, flex_props.clone()),
                    make_flexbox_cell_data_struct(layout_info_v, flex_props),
                )));
            }
        }
        let cells_h = llr_Expression::ReadLocalVariable {
            name: "cells_h".into(),
            ty: Type::Array(Rc::new(crate::typeregister::flexbox_layout_item_info_type())),
        };
        let cells_v = llr_Expression::ReadLocalVariable {
            name: "cells_v".into(),
            ty: Type::Array(Rc::new(crate::typeregister::flexbox_layout_item_info_type())),
        };
        FlexboxLayoutDataResult {
            alignment,
            direction,
            align_content,
            cross_axis_alignment,
            flex_wrap,
            cells_h,
            cells_v,
            compute_cells: Some(("cells_h".into(), "cells_v".into(), elements)),
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
    let e = crate::typeregister::BUILTIN.with(|e| e.enums.FlexboxLayoutAlignSelf.clone());
    (
        Type::Enumeration(e.clone()),
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        }),
    )
}

fn make_layout_cell_data_struct(layout_info: llr_Expression) -> llr_Expression {
    make_struct(
        BuiltinStruct::LayoutItemInfo,
        [("constraint", crate::typeregister::layout_info_type().into(), layout_info)],
    )
}

#[derive(Clone)]
struct FlexItemProps {
    grow: llr_Expression,
    shrink: llr_Expression,
    basis: llr_Expression,
    align_self: llr_Expression,
    order: llr_Expression,
}

fn make_flexbox_cell_data_struct(layout_info: llr_Expression, fp: FlexItemProps) -> llr_Expression {
    let (align_self_ty, _) = default_align_self();
    make_struct(
        BuiltinStruct::FlexboxLayoutItemInfo,
        [
            ("constraint", crate::typeregister::layout_info_type().into(), layout_info),
            ("flex-grow", Type::Float32, fp.grow),
            ("flex-shrink", Type::Float32, fp.shrink),
            ("flex-basis", Type::Float32, fp.basis),
            ("flex-align-self", align_self_ty, fp.align_self),
            ("flex-order", Type::Int32, fp.order),
        ],
    )
}

fn box_layout_data(
    layout: &crate::layout::BoxLayout,
    orientation: Orientation,
    ctx: &mut ExpressionLoweringCtx,
    cross_axis_size_override: Option<&crate::expression_tree::Expression>,
    cross_clamp: Option<&crate::expression_tree::Expression>,
) -> BoxLayoutDataResult {
    let alignment = if let Some(expr) = &layout.geometry.alignment {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.with(|e| e.enums.LayoutAlignment.clone());
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let repeater_count =
        layout.elems.iter().filter(|i| i.element.borrow().repeated.is_some()).count();

    let element_ty = crate::typeregister::layout_item_info_type();

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
                    );
                    make_layout_cell_data_struct(layout_info)
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
                }))
            } else {
                let layout_info = cell_layout_info(
                    &item.element,
                    &item.constraints,
                    ctx,
                    orientation,
                    cross_axis_size_override,
                    cross_clamp,
                );
                elements.push(Either::Left(make_layout_cell_data_struct(layout_info)));
            }
        }
        let cells = llr_Expression::ReadLocalVariable {
            name: "cells".into(),
            ty: Type::Array(Rc::new(crate::typeregister::layout_info_type().into())),
        };
        BoxLayoutDataResult { alignment, cells, compute_cells: Some(("cells".into(), elements)) }
    }
}

fn cell_layout_info(
    elem: &ElementRc,
    constraints: &crate::layout::LayoutConstraints,
    ctx: &mut ExpressionLoweringCtx,
    orientation: Orientation,
    cross_axis_size_override: Option<&crate::expression_tree::Expression>,
    cross_clamp: Option<&crate::expression_tree::Expression>,
) -> llr_Expression {
    let constraint = match orientation {
        Orientation::Vertical => {
            cross_axis_size_override.filter(|_| is_height_for_width_cell(elem)).cloned()
        }
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
    let array_ty = Type::Array(Rc::new(crate::typeregister::flexbox_layout_item_info_type()));
    let cells_expr = match &fld.compute_cells {
        Some((cells_h_var, cells_v_var, _)) => {
            let cells_var = match orientation {
                Orientation::Horizontal => cells_h_var.clone(),
                Orientation::Vertical => cells_v_var.clone(),
            };
            llr_Expression::ReadLocalVariable { name: cells_var.into(), ty: array_ty }
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
        Some((cells_h_variable, cells_v_variable, elements)) => {
            llr_Expression::WithFlexboxLayoutItemInfo {
                cells_h_variable,
                cells_v_variable,
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
            let wrap_enum =
                crate::typeregister::BUILTIN.with(|e| e.enums.FlexboxLayoutWrap.clone());
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
            let direction_enum =
                crate::typeregister::BUILTIN.with(|e| e.enums.FlexboxLayoutDirection.clone());
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
                    );
                    make_layout_cell_data_struct(layout_info)
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
                elements.push(Either::Right(LayoutRepeatedElement {
                    repeater_index,
                    row_child_templates,
                }));
            } else {
                let layout_info = cell_layout_info(
                    &item.item.element,
                    &item.item.constraints,
                    ctx,
                    orientation,
                    cross_axis_size_override,
                    None,
                );
                elements.push(Either::Left(make_layout_cell_data_struct(layout_info)));
            }
        }
        let cells = llr_Expression::ReadLocalVariable {
            name: "cells".into(),
            ty: Type::Array(Rc::new(crate::typeregister::layout_info_type().into())),
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
            ty: Type::Array(Rc::new(element_ty)),
        };
        GridLayoutInputDataResult { cells, compute_cells: Some(("cells".into(), elements)) }
    }
}

pub(super) fn grid_layout_input_data_ty() -> Type {
    Type::Struct(Rc::new(Struct::new(
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

    let (align_self_ty, align_self_default) = default_align_self();

    let grow = prop_ref("flex-grow").unwrap_or(llr_Expression::NumberLiteral(0.0));
    let shrink = prop_ref("flex-shrink").unwrap_or(llr_Expression::NumberLiteral(1.0));
    let basis = prop_ref("flex-basis").unwrap_or(llr_Expression::NumberLiteral(-1.0));
    let align_self = prop_ref("flex-align-self").unwrap_or(align_self_default);
    let order = prop_ref("flex-order").unwrap_or(llr_Expression::NumberLiteral(0.0));

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
            ("flex-grow", Type::Float32, grow),
            ("flex-shrink", Type::Float32, shrink),
            ("flex-basis", Type::Float32, basis),
            ("flex-align-self", align_self_ty, align_self),
            ("flex-order", Type::Int32, order),
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
    Some(get_layout_info(element, ctx, constraints, Orientation::Vertical, Some(width_constraint)))
}

/// Name of the local that carries the cross-axis (container) width into the
/// generated `flexbox_layout_item_info_at_cross_width` method body.
pub const FLEX_CROSS_WIDTH_LOCAL: &str = "flex_cross_width";

/// Like [`get_layout_info_v_constrained_for_repeated`], but measures at the
/// width passed in the [`FLEX_CROSS_WIDTH_LOCAL`] local instead of the
/// element's preferred width. A column FlexboxLayout supplies its real
/// container width here at solve time, so a repeated height-for-width instance
/// gets the same wrapped height as an equivalent static cell. Returns `None`
/// when the element has no constrained vertical layout-info.
pub fn get_layout_info_v_at_cross_width_for_repeated(
    ctx: &mut ExpressionLoweringCtx,
    element: &ElementRc,
    constraints: &crate::layout::LayoutConstraints,
) -> Option<llr_Expression> {
    if !element.borrow().has_inherited_layout_info_v_with_constraint() {
        return None;
    }
    let width_constraint = crate::expression_tree::Expression::ReadLocalVariable {
        name: FLEX_CROSS_WIDTH_LOCAL.into(),
        ty: Type::LogicalLength,
    };
    Some(get_layout_info(element, ctx, constraints, Orientation::Vertical, Some(width_constraint)))
}
