// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::BTreeMap;
use std::rc::Rc;

use itertools::Either;
use smol_str::SmolStr;

use super::lower_to_item_tree::LoweredElement;
use super::{GridLayoutRepeatedElement, LayoutRepeatedElement};
use crate::langtype::{BuiltinPrivateStruct, EnumerationValue, Struct, Type};
use crate::layout::{GridLayoutCell, Orientation, RowColExpr};
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
) -> llr_Expression {
    let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
    let organized_cells = ctx.map_property_reference(layout_organized_data_prop);
    let constraints_result = grid_layout_cell_constraints(layout, o, ctx);
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
) -> llr_Expression {
    let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
    let bld = box_layout_data(layout, o, ctx);
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
        BuiltinPrivateStruct::GridLayoutData,
        [
            ("size", Type::Float32, size),
            ("spacing", Type::Float32, spacing),
            ("padding", padding.ty(ctx), padding),
            ("organized_data", Type::ArrayOfU16, llr_Expression::PropertyReference(cells)),
        ],
    );
    let constraints_result = grid_layout_cell_constraints(layout, o, ctx);

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
    let bld = box_layout_data(layout, o, ctx);
    let size = layout_geometry_size(&layout.geometry.rect, o, ctx);
    let data = make_struct(
        BuiltinPrivateStruct::BoxLayoutData,
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
    match bld.compute_cells {
        Some((cells_variable, elements)) => llr_Expression::WithLayoutItemInfo {
            cells_variable,
            repeater_indices_var_name: Some("repeated_indices".into()),
            repeater_steps_var_name: None,
            elements,
            orientation: o,
            sub_expression: Box::new(llr_Expression::ExtraBuiltinFunctionCall {
                function: "solve_box_layout".into(),
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
            function: "solve_box_layout".into(),
            arguments: vec![data, empty_int32_slice()],
            return_ty: Type::LayoutCache,
        },
    }
}

pub(super) fn solve_flexbox_layout(
    layout: &crate::layout::FlexBoxLayout,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    let (padding_h, spacing_h) =
        generate_layout_padding_and_spacing(&layout.geometry, Orientation::Horizontal, ctx);
    let (padding_v, spacing_v) =
        generate_layout_padding_and_spacing(&layout.geometry, Orientation::Vertical, ctx);
    let fld = flexbox_layout_data(layout, ctx);
    let width = layout_geometry_size(&layout.geometry.rect, Orientation::Horizontal, ctx);
    let height = layout_geometry_size(&layout.geometry.rect, Orientation::Vertical, ctx);
    let data = make_struct(
        BuiltinPrivateStruct::FlexBoxLayoutData,
        [
            ("width", Type::Float32, width),
            ("height", Type::Float32, height),
            ("spacing_h", Type::Float32, spacing_h),
            ("spacing_v", Type::Float32, spacing_v),
            ("padding_h", padding_h.ty(ctx), padding_h),
            ("padding_v", padding_v.ty(ctx), padding_v),
            (
                "direction",
                crate::typeregister::BUILTIN
                    .with(|e| Type::Enumeration(e.enums.FlexDirection.clone())),
                fld.direction,
            ),
            ("cells_h", fld.cells_h.ty(ctx), fld.cells_h),
            ("cells_v", fld.cells_v.ty(ctx), fld.cells_v),
        ],
    );
    match fld.compute_cells {
        Some((cells_h_var, cells_v_var, elements)) => llr_Expression::WithFlexBoxLayoutItemInfo {
            cells_h_variable: cells_h_var,
            cells_v_variable: cells_v_var,
            repeater_indices_var_name: Some("repeated_indices".into()),
            elements,
            sub_expression: Box::new(llr_Expression::ExtraBuiltinFunctionCall {
                function: "solve_flexbox_layout".into(),
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
            function: "solve_flexbox_layout".into(),
            arguments: vec![data, empty_int32_slice()],
            return_ty: Type::LayoutCache,
        },
    }
}

pub(super) fn compute_flexbox_layout_info(
    layout: &crate::layout::FlexBoxLayout,
    orientation: Orientation,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    let fld = flexbox_layout_data(layout, ctx);

    // Try to determine direction at compile time from constant binding.
    let compile_time_direction =
        match layout.direction.as_ref() {
            None => Some(crate::layout::FlexDirection::Row),
            Some(nr) => nr.element().borrow().bindings.get(nr.name()).and_then(|binding| {
                match &binding.borrow().expression {
                    crate::expression_tree::Expression::EnumerationValue(ev) => match ev.value {
                        0 => Some(crate::layout::FlexDirection::Row),
                        1 => Some(crate::layout::FlexDirection::RowReverse),
                        2 => Some(crate::layout::FlexDirection::Column),
                        3 => Some(crate::layout::FlexDirection::ColumnReverse),
                        _ => None,
                    },
                    _ => None,
                }
            }),
        };

    if let Some(direction) = compile_time_direction {
        // If direction is known at compile time, we can optimize by only generating
        let is_cross_axis = matches!(
            (direction, orientation),
            (crate::layout::FlexDirection::Row, Orientation::Vertical)
                | (crate::layout::FlexDirection::RowReverse, Orientation::Vertical)
                | (crate::layout::FlexDirection::Column, Orientation::Horizontal)
                | (crate::layout::FlexDirection::ColumnReverse, Orientation::Horizontal)
        );
        compute_flexbox_layout_info_for_direction(layout, orientation, is_cross_axis, fld, ctx)
    } else {
        // Direction is not known at compile time - generate runtime conditional
        // This ensures we only read the constraint (width/height) in the branch where it's needed

        let row_expr = compute_flexbox_layout_info_for_direction(
            layout,
            orientation,
            orientation == Orientation::Vertical, // cross-axis if orientation is vertical
            fld.clone(),
            ctx,
        );
        let col_expr = compute_flexbox_layout_info_for_direction(
            layout,
            orientation,
            orientation == Orientation::Horizontal, // cross-axis if orientation is horizontal
            fld,
            ctx,
        );

        // Condition: direction == Row || direction == RowReverse
        let direction_enum = crate::typeregister::BUILTIN.with(|e| e.enums.FlexDirection.clone());
        let direction_ref = llr_Expression::PropertyReference(
            ctx.map_property_reference(layout.direction.as_ref().unwrap()),
        );

        let is_row_condition = llr_Expression::BinaryExpression {
            lhs: Box::new(llr_Expression::BinaryExpression {
                lhs: Box::new(direction_ref.clone()),
                rhs: Box::new(llr_Expression::EnumerationValue(EnumerationValue {
                    value: 0, // FlexDirection::Row
                    enumeration: direction_enum.clone(),
                })),
                op: '=',
            }),
            rhs: Box::new(llr_Expression::BinaryExpression {
                lhs: Box::new(direction_ref),
                rhs: Box::new(llr_Expression::EnumerationValue(EnumerationValue {
                    value: 1, // FlexDirection::RowReverse
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

fn compute_flexbox_layout_info_for_direction(
    layout: &crate::layout::FlexBoxLayout,
    orientation: Orientation,
    is_cross_axis: bool,
    fld: FlexBoxLayoutDataResult,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    let (padding_h, spacing_h) =
        generate_layout_padding_and_spacing(&layout.geometry, Orientation::Horizontal, ctx);
    let (padding_v, spacing_v) =
        generate_layout_padding_and_spacing(&layout.geometry, Orientation::Vertical, ctx);

    if is_cross_axis {
        // Cross-axis: need constraint to handle wrapping

        // For cross-axis, pass the perpendicular dimension as constraint
        let constraint_size = match orientation {
            Orientation::Horizontal => {
                layout_geometry_size(&layout.geometry.rect, Orientation::Vertical, ctx)
            }
            Orientation::Vertical => {
                layout_geometry_size(&layout.geometry.rect, Orientation::Horizontal, ctx)
            }
        };

        let orientation_expr = llr_Expression::EnumerationValue(EnumerationValue {
            value: orientation as usize,
            enumeration: crate::typeregister::BUILTIN.with(|e| e.enums.Orientation.clone()),
        });

        let arguments = vec![
            fld.cells_h,
            fld.cells_v,
            spacing_h,
            spacing_v,
            padding_h,
            padding_v,
            orientation_expr,
            fld.direction,
            constraint_size,
        ];

        match fld.compute_cells {
            Some((cells_h_var, cells_v_var, elements)) => {
                llr_Expression::WithFlexBoxLayoutItemInfo {
                    cells_h_variable: cells_h_var,
                    cells_v_variable: cells_v_var,
                    repeater_indices_var_name: None,
                    elements,
                    sub_expression: Box::new(llr_Expression::ExtraBuiltinFunctionCall {
                        function: "flexbox_layout_info".into(),
                        arguments,
                        return_ty: crate::typeregister::layout_info_type().into(),
                    }),
                }
            }
            None => llr_Expression::ExtraBuiltinFunctionCall {
                function: "flexbox_layout_info".into(),
                arguments,
                return_ty: crate::typeregister::layout_info_type().into(),
            },
        }
    } else {
        // Main axis: determine minimum size
        let arguments = vec![
            fld.cells_h,
            fld.cells_v,
            spacing_h,
            spacing_v,
            padding_h,
            padding_v,
            llr_Expression::EnumerationValue(EnumerationValue {
                value: orientation as usize,
                enumeration: crate::typeregister::BUILTIN.with(|e| e.enums.Orientation.clone()),
            }),
            fld.direction,
            llr_Expression::NumberLiteral(f32::MAX.into()),
        ];

        match fld.compute_cells {
            Some((cells_h_var, cells_v_var, elements)) => {
                llr_Expression::WithFlexBoxLayoutItemInfo {
                    cells_h_variable: cells_h_var,
                    cells_v_variable: cells_v_var,
                    repeater_indices_var_name: None,
                    elements,
                    sub_expression: Box::new(llr_Expression::ExtraBuiltinFunctionCall {
                        function: "flexbox_layout_info".into(),
                        arguments,
                        return_ty: crate::typeregister::layout_info_type().into(),
                    }),
                }
            }
            None => llr_Expression::ExtraBuiltinFunctionCall {
                function: "flexbox_layout_info".into(),
                arguments,
                return_ty: crate::typeregister::layout_info_type().into(),
            },
        }
    }
}

#[derive(Clone)]
struct FlexBoxLayoutDataResult {
    direction: llr_Expression,
    cells_h: llr_Expression,
    cells_v: llr_Expression,
    /// When there are repeaters involved, we need to do a WithFlexBoxLayoutItemInfo with the
    /// given cells_h/cells_v variable names and elements (each static element has a tuple of (h, v) layout info)
    compute_cells: Option<(
        String,
        String,
        Vec<Either<(llr_Expression, llr_Expression), LayoutRepeatedElement>>,
    )>,
}

fn flexbox_layout_data(
    layout: &crate::layout::FlexBoxLayout,
    ctx: &mut ExpressionLoweringCtx,
) -> FlexBoxLayoutDataResult {
    let direction = if let Some(expr) = &layout.direction {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.with(|e| e.enums.FlexDirection.clone());
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let repeater_count =
        layout.elems.iter().filter(|i| i.element.borrow().repeated.is_some()).count();

    let element_ty = crate::typeregister::box_layout_cell_data_type();

    if repeater_count == 0 {
        let cells_h = llr_Expression::Array {
            values: layout
                .elems
                .iter()
                .map(|li| {
                    let layout_info_h =
                        get_layout_info(&li.element, ctx, &li.constraints, Orientation::Horizontal);
                    make_layout_cell_data_struct(layout_info_h)
                })
                .collect(),
            element_ty: element_ty.clone(),
            output: llr_ArrayOutput::Slice,
        };
        let cells_v = llr_Expression::Array {
            values: layout
                .elems
                .iter()
                .map(|li| {
                    let layout_info_v =
                        get_layout_info(&li.element, ctx, &li.constraints, Orientation::Vertical);
                    make_layout_cell_data_struct(layout_info_v)
                })
                .collect(),
            element_ty,
            output: llr_ArrayOutput::Slice,
        };
        FlexBoxLayoutDataResult { direction, cells_h, cells_v, compute_cells: None }
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
                    repeated_children_count: None,
                }))
            } else {
                // For static elements, we need both orientations
                let layout_info_h =
                    get_layout_info(&item.element, ctx, &item.constraints, Orientation::Horizontal);
                let layout_info_v =
                    get_layout_info(&item.element, ctx, &item.constraints, Orientation::Vertical);
                elements.push(Either::Left((
                    make_layout_cell_data_struct(layout_info_h),
                    make_layout_cell_data_struct(layout_info_v),
                )));
            }
        }
        let cells_h = llr_Expression::ReadLocalVariable {
            name: "cells_h".into(),
            ty: Type::Array(Rc::new(crate::typeregister::layout_info_type().into())),
        };
        let cells_v = llr_Expression::ReadLocalVariable {
            name: "cells_v".into(),
            ty: Type::Array(Rc::new(crate::typeregister::layout_info_type().into())),
        };
        FlexBoxLayoutDataResult {
            direction,
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

fn make_layout_cell_data_struct(layout_info: llr_Expression) -> llr_Expression {
    make_struct(
        BuiltinPrivateStruct::LayoutItemInfo,
        [("constraint", crate::typeregister::layout_info_type().into(), layout_info)],
    )
}

fn box_layout_data(
    layout: &crate::layout::BoxLayout,
    orientation: Orientation,
    ctx: &mut ExpressionLoweringCtx,
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

    let element_ty = crate::typeregister::box_layout_cell_data_type();

    if repeater_count == 0 {
        let cells = llr_Expression::Array {
            values: layout
                .elems
                .iter()
                .map(|li| {
                    let layout_info =
                        get_layout_info(&li.element, ctx, &li.constraints, orientation);
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
                    repeated_children_count: None,
                }))
            } else {
                let layout_info =
                    get_layout_info(&item.element, ctx, &item.constraints, orientation);
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
) -> GridLayoutCellConstraintsResult {
    let repeater_count =
        layout.elems.iter().filter(|i| i.item.element.borrow().repeated.is_some()).count();

    let element_ty = crate::typeregister::box_layout_cell_data_type();

    if repeater_count == 0 {
        let cells = llr_Expression::Array {
            element_ty,
            values: layout
                .elems
                .iter()
                .map(|li| {
                    let layout_info =
                        get_layout_info(&li.item.element, ctx, &li.item.constraints, orientation);
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
                let cell = item.cell.borrow();
                let repeated_children_count = cell.child_items.as_ref().map(|c| c.len());
                elements.push(Either::Right(LayoutRepeatedElement {
                    repeater_index,
                    repeated_children_count,
                }));
            } else {
                let layout_info =
                    get_layout_info(&item.item.element, ctx, &item.item.constraints, orientation);
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
            BuiltinPrivateStruct::GridLayoutInputData,
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
                let cell = item.cell.borrow();
                let repeated_children_count = cell.child_items.as_ref().map(|c| c.len());
                let repeated_element =
                    GridLayoutRepeatedElement { new_row, repeater_index, repeated_children_count };
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
    Type::Struct(Rc::new(Struct {
        fields: IntoIterator::into_iter([
            (SmolStr::new_static("new_row"), Type::Bool),
            (SmolStr::new_static("row"), Type::Int32),
            (SmolStr::new_static("col"), Type::Int32),
            (SmolStr::new_static("rowspan"), Type::Int32),
            (SmolStr::new_static("colspan"), Type::Int32),
        ])
        .collect(),
        name: BuiltinPrivateStruct::GridLayoutInputData.into(),
    }))
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
        BuiltinPrivateStruct::Padding,
        [("begin", Type::Float32, padding_prop(begin)), ("end", Type::Float32, padding_prop(end))],
    );

    (padding, spacing)
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
) -> llr_Expression {
    let layout_info = if let Some(layout_info_prop) = &elem.borrow().layout_info_prop(orientation) {
        llr_Expression::PropertyReference(ctx.map_property_reference(layout_info_prop))
    } else {
        super::lower_expression::lower_expression(
            &crate::layout::implicit_layout_info_call(elem, orientation),
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
                BuiltinPrivateStruct::GridLayoutInputData,
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
        // Repeated Row
        let mut new_row_expr = llr_Expression::BoolLiteral(true);
        for (i, child_item) in child_items.iter().enumerate() {
            let child_element = child_item.element.borrow();
            let child_cell = child_element.grid_layout_cell.as_ref().unwrap().borrow();
            push_assignment(i, &new_row_expr, &child_cell);
            new_row_expr = llr_Expression::BoolLiteral(false);
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
