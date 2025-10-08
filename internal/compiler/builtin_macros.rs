// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the implementation of the builtin macros.
//! They are just transformations that convert into some more complicated expression tree

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{
    BuiltinFunction, BuiltinMacroFunction, Callable, EasingCurve, Expression, MinMaxOp, Unit,
};
use crate::langtype::{EnumerationValue, Type};
use crate::parser::NodeOrToken;
use smol_str::{format_smolstr, ToSmolStr};

/// Used for uniquely name some variables
static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

/// "Expand" the macro `mac` (at location `n`) with the arguments `sub_expr`
pub fn lower_macro(
    mac: BuiltinMacroFunction,
    n: &dyn Spanned,
    mut sub_expr: impl Iterator<Item = (Expression, Option<NodeOrToken>)>,
    diag: &mut BuildDiagnostics,
) -> Expression {
    match mac {
        BuiltinMacroFunction::Min => min_max_macro(n, MinMaxOp::Min, sub_expr.collect(), diag),
        BuiltinMacroFunction::Max => min_max_macro(n, MinMaxOp::Max, sub_expr.collect(), diag),
        BuiltinMacroFunction::Clamp => clamp_macro(n, sub_expr.collect(), diag),
        BuiltinMacroFunction::Mod => mod_macro(n, sub_expr.collect(), diag),
        BuiltinMacroFunction::Abs => abs_macro(n, sub_expr.collect(), diag),
        BuiltinMacroFunction::Sign => {
            let Some((x, arg_node)) = sub_expr.next() else {
                diag.push_error("Expected one argument".into(), n);
                return Expression::Invalid;
            };
            if sub_expr.next().is_some() {
                diag.push_error("Expected only one argument".into(), n);
            }
            Expression::Condition {
                condition: Expression::BinaryExpression {
                    lhs: x.maybe_convert_to(Type::Float32, &arg_node, diag).into(),
                    rhs: Expression::NumberLiteral(0., Unit::None).into(),
                    op: '<',
                }
                .into(),
                true_expr: Expression::NumberLiteral(-1., Unit::None).into(),
                false_expr: Expression::NumberLiteral(1., Unit::None).into(),
            }
        }
        BuiltinMacroFunction::Debug => debug_macro(n, sub_expr.collect(), diag),
        BuiltinMacroFunction::CubicBezier => {
            let mut has_error = None;
            let expected_argument_type_error =
                "Arguments to cubic bezier curve must be number literal";
            // FIXME: this is not pretty to be handling there.
            // Maybe "cubic_bezier" should be a function that is lowered later
            let mut a = || match sub_expr.next() {
                None => {
                    has_error.get_or_insert((n.to_source_location(), "Not enough arguments"));
                    0.
                }
                Some((Expression::NumberLiteral(val, Unit::None), _)) => val as f32,
                // handle negative numbers
                Some((Expression::UnaryOp { sub, op: '-' }, n)) => match *sub {
                    Expression::NumberLiteral(val, Unit::None) => (-1.0 * val) as f32,
                    _ => {
                        has_error
                            .get_or_insert((n.to_source_location(), expected_argument_type_error));
                        0.
                    }
                },
                Some((_, n)) => {
                    has_error.get_or_insert((n.to_source_location(), expected_argument_type_error));
                    0.
                }
            };
            let expr = Expression::EasingCurve(EasingCurve::CubicBezier(a(), a(), a(), a()));
            if let Some((_, n)) = sub_expr.next() {
                has_error
                    .get_or_insert((n.to_source_location(), "Too many argument for bezier curve"));
            }
            if let Some((n, msg)) = has_error {
                diag.push_error(msg.into(), &n);
            }

            expr
        }
        BuiltinMacroFunction::Rgb => rgb_macro(n, sub_expr.collect(), diag),
        BuiltinMacroFunction::Hsv => hsv_macro(n, sub_expr.collect(), diag),
    }
}

fn min_max_macro(
    node: &dyn Spanned,
    op: MinMaxOp,
    args: Vec<(Expression, Option<NodeOrToken>)>,
    diag: &mut BuildDiagnostics,
) -> Expression {
    if args.is_empty() {
        diag.push_error("Needs at least one argument".into(), node);
        return Expression::Invalid;
    }
    let ty = Expression::common_target_type_for_type_list(args.iter().map(|expr| expr.0.ty()));
    if ty.as_unit_product().is_none() {
        diag.push_error("Invalid argument type".into(), node);
        return Expression::Invalid;
    }
    let mut args = args.into_iter();
    let (base, arg_node) = args.next().unwrap();
    let mut base = base.maybe_convert_to(ty.clone(), &arg_node, diag);
    for (next, arg_node) in args {
        let rhs = next.maybe_convert_to(ty.clone(), &arg_node, diag);
        base = min_max_expression(base, rhs, op);
    }
    base
}

fn clamp_macro(
    node: &dyn Spanned,
    args: Vec<(Expression, Option<NodeOrToken>)>,
    diag: &mut BuildDiagnostics,
) -> Expression {
    if args.len() != 3 {
        diag.push_error(
            "`clamp` needs three values: the `value` to clamp, the `minimum` and the `maximum`"
                .into(),
            node,
        );
        return Expression::Invalid;
    }
    let (value, value_node) = args.first().unwrap().clone();
    let ty = value.ty();
    if ty.as_unit_product().is_none() {
        diag.push_error("Invalid argument type".into(), &value_node);
        return Expression::Invalid;
    }

    let (min, min_node) = args.get(1).unwrap().clone();
    let min = min.maybe_convert_to(ty.clone(), &min_node, diag);
    let (max, max_node) = args.get(2).unwrap().clone();
    let max = max.maybe_convert_to(ty.clone(), &max_node, diag);

    let value = min_max_expression(value, max, MinMaxOp::Min);
    min_max_expression(min, value, MinMaxOp::Max)
}

fn mod_macro(
    node: &dyn Spanned,
    args: Vec<(Expression, Option<NodeOrToken>)>,
    diag: &mut BuildDiagnostics,
) -> Expression {
    if args.len() != 2 {
        diag.push_error("Needs 2 arguments".into(), node);
        return Expression::Invalid;
    }
    let (lhs_ty, rhs_ty) = (args[0].0.ty(), args[1].0.ty());
    let common_ty = if lhs_ty.default_unit().is_some() {
        lhs_ty
    } else if rhs_ty.default_unit().is_some() {
        rhs_ty
    } else if matches!(lhs_ty, Type::UnitProduct(_)) {
        lhs_ty
    } else if matches!(rhs_ty, Type::UnitProduct(_)) {
        rhs_ty
    } else {
        Type::Float32
    };

    let source_location = Some(node.to_source_location());
    let function = Callable::Builtin(BuiltinFunction::Mod);
    let arguments = args.into_iter().map(|(e, n)| e.maybe_convert_to(common_ty.clone(), &n, diag));
    if matches!(common_ty, Type::Float32) {
        Expression::FunctionCall { function, arguments: arguments.collect(), source_location }
    } else {
        Expression::Cast {
            from: Expression::FunctionCall {
                function,
                arguments: arguments
                    .map(|a| Expression::Cast { from: a.into(), to: Type::Float32 })
                    .collect(),
                source_location,
            }
            .into(),
            to: common_ty.clone(),
        }
    }
}

fn abs_macro(
    node: &dyn Spanned,
    args: Vec<(Expression, Option<NodeOrToken>)>,
    diag: &mut BuildDiagnostics,
) -> Expression {
    if args.len() != 1 {
        diag.push_error("Needs 1 argument".into(), node);
        return Expression::Invalid;
    }
    let ty = args[0].0.ty();
    let ty = if ty.default_unit().is_some() || matches!(ty, Type::UnitProduct(_)) {
        ty
    } else {
        Type::Float32
    };

    let source_location = Some(node.to_source_location());
    let function = Callable::Builtin(BuiltinFunction::Abs);
    if matches!(ty, Type::Float32) {
        let arguments =
            args.into_iter().map(|(e, n)| e.maybe_convert_to(ty.clone(), &n, diag)).collect();
        Expression::FunctionCall { function, arguments, source_location }
    } else {
        Expression::Cast {
            from: Expression::FunctionCall {
                function,
                arguments: args
                    .into_iter()
                    .map(|(a, _)| Expression::Cast { from: a.into(), to: Type::Float32 })
                    .collect(),
                source_location,
            }
            .into(),
            to: ty,
        }
    }
}

fn rgb_macro(
    node: &dyn Spanned,
    args: Vec<(Expression, Option<NodeOrToken>)>,
    diag: &mut BuildDiagnostics,
) -> Expression {
    if args.len() < 3 || args.len() > 4 {
        diag.push_error(
            format!("This function needs 3 or 4 arguments, but {} were provided", args.len()),
            node,
        );
        return Expression::Invalid;
    }
    let mut arguments: Vec<_> = args
        .into_iter()
        .enumerate()
        .map(|(i, (expr, n))| {
            if i < 3 {
                if expr.ty() == Type::Percent {
                    Expression::BinaryExpression {
                        lhs: Box::new(expr.maybe_convert_to(Type::Float32, &n, diag)),
                        rhs: Box::new(Expression::NumberLiteral(255., Unit::None)),
                        op: '*',
                    }
                } else {
                    expr.maybe_convert_to(Type::Float32, &n, diag)
                }
            } else {
                expr.maybe_convert_to(Type::Float32, &n, diag)
            }
        })
        .collect();
    if arguments.len() < 4 {
        arguments.push(Expression::NumberLiteral(1., Unit::None))
    }
    Expression::FunctionCall {
        function: BuiltinFunction::Rgb.into(),
        arguments,
        source_location: Some(node.to_source_location()),
    }
}

fn hsv_macro(
    node: &dyn Spanned,
    args: Vec<(Expression, Option<NodeOrToken>)>,
    diag: &mut BuildDiagnostics,
) -> Expression {
    if args.len() < 3 || args.len() > 4 {
        diag.push_error(
            format!("This function needs 3 or 4 arguments, but {} were provided", args.len()),
            node,
        );
        return Expression::Invalid;
    }
    let mut arguments: Vec<_> =
        args.into_iter().map(|(expr, n)| expr.maybe_convert_to(Type::Float32, &n, diag)).collect();
    if arguments.len() < 4 {
        arguments.push(Expression::NumberLiteral(1., Unit::None))
    }
    Expression::FunctionCall {
        function: BuiltinFunction::Hsv.into(),
        arguments,
        source_location: Some(node.to_source_location()),
    }
}

fn debug_macro(
    node: &dyn Spanned,
    args: Vec<(Expression, Option<NodeOrToken>)>,
    diag: &mut BuildDiagnostics,
) -> Expression {
    let mut string = None;
    for (expr, node) in args {
        let val = to_debug_string(expr, &node, diag);
        string = Some(match string {
            None => val,
            Some(string) => Expression::BinaryExpression {
                lhs: Box::new(string),
                op: '+',
                rhs: Box::new(Expression::BinaryExpression {
                    lhs: Box::new(Expression::StringLiteral(" ".into())),
                    op: '+',
                    rhs: Box::new(val),
                }),
            },
        });
    }
    Expression::FunctionCall {
        function: BuiltinFunction::Debug.into(),
        arguments: vec![string.unwrap_or_else(|| Expression::default_value_for_type(&Type::String))],
        source_location: Some(node.to_source_location()),
    }
}

fn to_debug_string(
    expr: Expression,
    node: &dyn Spanned,
    diag: &mut BuildDiagnostics,
) -> Expression {
    let ty = expr.ty();
    match &ty {
        Type::Invalid => Expression::Invalid,
        Type::Void
        | Type::InferredCallback
        | Type::InferredProperty
        | Type::Callback { .. }
        | Type::ComponentFactory
        | Type::Function { .. }
        | Type::ElementReference
        | Type::LayoutCache
        | Type::Model
        | Type::PathData => {
            diag.push_error("Cannot debug this expression".into(), node);
            Expression::Invalid
        }
        Type::Float32 | Type::Int32 => expr.maybe_convert_to(Type::String, node, diag),
        Type::String => expr,
        // TODO
        Type::Color | Type::Brush | Type::Image | Type::Easing | Type::Array(_) => {
            Expression::StringLiteral("<debug-of-this-type-not-yet-implemented>".into())
        }
        Type::Duration
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Rem
        | Type::Angle
        | Type::Percent
        | Type::UnitProduct(_) => Expression::BinaryExpression {
            lhs: Box::new(
                Expression::Cast { from: Box::new(expr), to: Type::Float32 }.maybe_convert_to(
                    Type::String,
                    node,
                    diag,
                ),
            ),
            op: '+',
            rhs: Box::new(Expression::StringLiteral(
                Type::UnitProduct(ty.as_unit_product().unwrap()).to_smolstr(),
            )),
        },
        Type::Bool => Expression::Condition {
            condition: Box::new(expr),
            true_expr: Box::new(Expression::StringLiteral("true".into())),
            false_expr: Box::new(Expression::StringLiteral("false".into())),
        },
        Type::Struct(s) => {
            let local_object = format_smolstr!(
                "debug_struct{}",
                COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            );
            let mut string = None;
            for k in s.fields.keys() {
                let field_name = if string.is_some() {
                    format_smolstr!(", {}: ", k)
                } else {
                    format_smolstr!("{{ {}: ", k)
                };
                let value = to_debug_string(
                    Expression::StructFieldAccess {
                        base: Box::new(Expression::ReadLocalVariable {
                            name: local_object.clone(),
                            ty: ty.clone(),
                        }),
                        name: k.clone(),
                    },
                    node,
                    diag,
                );
                let field = Expression::BinaryExpression {
                    lhs: Box::new(Expression::StringLiteral(field_name)),
                    op: '+',
                    rhs: Box::new(value),
                };
                string = Some(match string {
                    None => field,
                    Some(x) => Expression::BinaryExpression {
                        lhs: Box::new(x),
                        op: '+',
                        rhs: Box::new(field),
                    },
                });
            }
            match string {
                None => Expression::StringLiteral("{}".into()),
                Some(string) => Expression::CodeBlock(vec![
                    Expression::StoreLocalVariable { name: local_object, value: Box::new(expr) },
                    Expression::BinaryExpression {
                        lhs: Box::new(string),
                        op: '+',
                        rhs: Box::new(Expression::StringLiteral(" }".into())),
                    },
                ]),
            }
        }
        Type::Enumeration(enu) => {
            let local_object = "debug_enum";
            let mut v = vec![Expression::StoreLocalVariable {
                name: local_object.into(),
                value: Box::new(expr),
            }];
            let mut cond =
                Expression::StringLiteral(format_smolstr!("Error: invalid value for {}", ty));
            for (idx, val) in enu.values.iter().enumerate() {
                cond = Expression::Condition {
                    condition: Box::new(Expression::BinaryExpression {
                        lhs: Box::new(Expression::ReadLocalVariable {
                            name: local_object.into(),
                            ty: ty.clone(),
                        }),
                        rhs: Box::new(Expression::EnumerationValue(EnumerationValue {
                            value: idx,
                            enumeration: enu.clone(),
                        })),
                        op: '=',
                    }),
                    true_expr: Box::new(Expression::StringLiteral(val.clone())),
                    false_expr: Box::new(cond),
                };
            }
            v.push(cond);
            Expression::CodeBlock(v)
        }
    }
}

/// Generate an expression which is like `min(lhs, rhs)` if op is '<' or `max(lhs, rhs)` if op is '>'.
/// counter is an unique id.
/// The rhs and lhs of the expression must have the same numerical type
pub fn min_max_expression(lhs: Expression, rhs: Expression, op: MinMaxOp) -> Expression {
    let lhs_ty = lhs.ty();
    let rhs_ty = rhs.ty();
    let ty = match (lhs_ty, rhs_ty) {
        (a, b) if a == b => a,
        (Type::Int32, Type::Float32) | (Type::Float32, Type::Int32) => Type::Float32,
        _ => Type::Invalid,
    };
    Expression::MinMax { ty, op, lhs: Box::new(lhs), rhs: Box::new(rhs) }
}
