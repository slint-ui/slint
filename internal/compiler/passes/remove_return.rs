// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use smol_str::{format_smolstr, SmolStr};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use crate::expression_tree::Expression;
use crate::langtype::{Struct, Type};

pub fn remove_return(doc: &crate::object_tree::Document) {
    doc.visit_all_used_components(|component| {
        crate::object_tree::visit_all_expressions(component, |e, _| {
            let mut ret_ty = None;
            fn visit(e: &Expression, ret_ty: &mut Option<Type>) {
                if ret_ty.is_some() {
                    return;
                }
                match e {
                    Expression::ReturnStatement(x) => {
                        *ret_ty = Some(x.as_ref().map_or(Type::Void, |x| x.ty()));
                    }
                    _ => e.visit(|e| visit(e, ret_ty)),
                };
            }
            visit(e, &mut ret_ty);
            let Some(ret_ty) = ret_ty else { return };
            let ctx = RemoveReturnContext { ret_ty };
            *e = process_expression(std::mem::take(e), &ctx).to_expression(&ctx.ret_ty);
        })
    });
}

fn process_expression(e: Expression, ctx: &RemoveReturnContext) -> ExpressionResult {
    let ty = e.ty();
    match e {
        Expression::ReturnStatement(expr) => ExpressionResult::Return(expr.map(|e| *e)),
        Expression::CodeBlock(expr) => process_codeblock(expr.into_iter().peekable(), &ty, ctx),
        Expression::Condition { condition, true_expr, false_expr } => {
            let te = process_expression(*true_expr, ctx);
            let fe = process_expression(*false_expr, ctx);
            match (te, fe) {
                (ExpressionResult::Just(te), ExpressionResult::Just(fe)) => {
                    Expression::Condition { condition, true_expr: te.into(), false_expr: fe.into() }
                        .into()
                }
                (ExpressionResult::Just(te), ExpressionResult::Return(fe)) => {
                    ExpressionResult::MaybeReturn {
                        pre_statements: vec![],
                        condition: *condition,
                        returned_value: fe,
                        actual_value: cleanup_empty_block(te),
                    }
                }
                (ExpressionResult::Return(te), ExpressionResult::Just(fe)) => {
                    ExpressionResult::MaybeReturn {
                        pre_statements: vec![],
                        condition: Expression::UnaryOp { sub: condition, op: '!' },
                        returned_value: te,
                        actual_value: cleanup_empty_block(fe),
                    }
                }
                (te, fe) => {
                    let te = te.into_return_object(&ty, &ctx.ret_ty);
                    let fe = fe.into_return_object(&ty, &ctx.ret_ty);
                    ExpressionResult::ReturnObject {
                        has_value: !matches!(ty, Type::Void | Type::Invalid),
                        has_return_value: !matches!(ctx.ret_ty, Type::Void | Type::Invalid),
                        value: Expression::Condition {
                            condition,
                            true_expr: te.into(),
                            false_expr: fe.into(),
                        },
                    }
                }
            }
        }
        Expression::Cast { from, to } => {
            process_expression(*from, ctx).map_value(|e| Expression::Cast { from: e.into(), to })
        }
        e => {
            // Normally there shouldn't be any 'return' statements in there since return are not allowed in arbitrary expressions
            ExpressionResult::Just(e)
        }
    }
}

/// Return the expression, unless it is an empty codeblock, then return None
fn cleanup_empty_block(te: Expression) -> Option<Expression> {
    if matches!(&te, Expression::CodeBlock(stmts) if stmts.is_empty()) {
        None
    } else {
        Some(te)
    }
}

fn process_codeblock(
    mut iter: std::iter::Peekable<impl Iterator<Item = Expression>>,
    ty: &Type,
    ctx: &RemoveReturnContext,
) -> ExpressionResult {
    let mut stmts = vec![];
    while let Some(e) = iter.next() {
        match process_expression(e, ctx) {
            ExpressionResult::Just(x) => stmts.push(x),
            ExpressionResult::Return(x) => {
                stmts.extend(x);
                return ExpressionResult::Return(
                    (!stmts.is_empty()).then_some(Expression::CodeBlock(stmts)),
                );
            }
            ExpressionResult::MaybeReturn {
                mut pre_statements,
                condition,
                returned_value,
                actual_value,
            } => {
                stmts.append(&mut pre_statements);
                if iter.peek().is_none() {
                    return ExpressionResult::MaybeReturn {
                        pre_statements: stmts,
                        condition,
                        returned_value,
                        actual_value,
                    };
                };
                return continue_codeblock(
                    iter,
                    ty,
                    ctx,
                    ExpressionResult::MaybeReturn {
                        pre_statements: vec![],
                        condition,
                        returned_value,
                        actual_value,
                    }
                    .into_return_object(ty, &ctx.ret_ty),
                    stmts,
                    !matches!(ctx.ret_ty, Type::Void | Type::Invalid),
                    !matches!(ty, Type::Void | Type::Invalid),
                );
            }
            ExpressionResult::ReturnObject { value, has_value, has_return_value } => {
                if iter.peek().is_none() {
                    return ExpressionResult::ReturnObject {
                        value: codeblock_with_expr(stmts, value),
                        has_value,
                        has_return_value,
                    };
                };
                return continue_codeblock(
                    iter,
                    ty,
                    ctx,
                    value,
                    stmts,
                    has_return_value,
                    !matches!(ty, Type::Void | Type::Invalid),
                );
            }
        }
    }
    ExpressionResult::Just(Expression::CodeBlock(stmts))
}

fn continue_codeblock(
    iter: std::iter::Peekable<impl Iterator<Item = Expression>>,
    ty: &Type,
    ctx: &RemoveReturnContext,
    return_object: Expression,
    mut stmts: Vec<Expression>,
    has_return_value: bool,
    has_value: bool,
) -> ExpressionResult {
    let rest = process_codeblock(iter, ty, ctx).into_return_object(ty, &ctx.ret_ty);
    static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let unique_name = format_smolstr!(
        "return_check_merge{}",
        COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let load = Box::new(Expression::ReadLocalVariable {
        name: unique_name.clone(),
        ty: return_object.ty(),
    });
    stmts.push(Expression::StoreLocalVariable { name: unique_name, value: return_object.into() });
    stmts.push(Expression::Condition {
        condition: Expression::StructFieldAccess {
            base: load.clone(),
            name: FIELD_CONDITION.into(),
        }
        .into(),
        true_expr: rest.into(),
        false_expr: ExpressionResult::Return(has_return_value.then(|| {
            Expression::StructFieldAccess { base: load.clone(), name: FIELD_RETURNED.into() }
        }))
        .into_return_object(ty, &ctx.ret_ty)
        .into(),
    });
    ExpressionResult::ReturnObject {
        value: Expression::CodeBlock(stmts),
        has_value,
        has_return_value,
    }
}

struct RemoveReturnContext {
    ret_ty: Type,
}

#[derive(Debug)]
enum ExpressionResult {
    /// The expression maps directly to a LLR expression
    Just(Expression),
    /// The expression used `return` so we need to check for the return slot
    MaybeReturn {
        /// Some statements that initializes some temporary variable (eg arguments to something called later)
        pre_statements: Vec<Expression>,
        /// Boolean expression: false means return
        condition: Expression,
        /// Value being returned if condition is false
        returned_value: Option<Expression>,
        /// The value when we don't return
        actual_value: Option<Expression>,
    },
    /// The expression returns unconditionally
    Return(Option<Expression>),
    /// The expression is of type `{ condition: bool, actual: ty, returned: ret_ty}`
    /// which is the equivalent of `if condition { actual } else { return R }`
    ReturnObject { value: Expression, has_value: bool, has_return_value: bool },
}

impl From<Expression> for ExpressionResult {
    fn from(v: Expression) -> Self {
        Self::Just(v)
    }
}

const FIELD_CONDITION: &str = "condition";
const FIELD_ACTUAL: &str = "actual";
const FIELD_RETURNED: &str = "returned";

impl ExpressionResult {
    fn to_expression(self, ty: &Type) -> Expression {
        match self {
            ExpressionResult::Just(e) => e,
            ExpressionResult::Return(e) => e.unwrap_or(Expression::CodeBlock(vec![])),
            ExpressionResult::MaybeReturn {
                mut pre_statements,
                condition,
                returned_value,
                actual_value,
            } => {
                pre_statements.push(Expression::Condition {
                    condition: condition.into(),
                    true_expr: actual_value.unwrap_or(Expression::CodeBlock(vec![])).into(),
                    false_expr: returned_value.unwrap_or(Expression::CodeBlock(vec![])).into(),
                });
                Expression::CodeBlock(pre_statements)
            }
            ExpressionResult::ReturnObject { value, has_value, has_return_value } => {
                static COUNT: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let name = format_smolstr!(
                    "returned_expression{}",
                    COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                );
                let load =
                    Box::new(Expression::ReadLocalVariable { name: name.clone(), ty: value.ty() });
                Expression::CodeBlock(vec![
                    Expression::StoreLocalVariable { name, value: value.into() },
                    Expression::Condition {
                        condition: Expression::StructFieldAccess {
                            base: load.clone(),
                            name: FIELD_CONDITION.into(),
                        }
                        .into(),
                        true_expr: if has_value {
                            Expression::StructFieldAccess {
                                base: load.clone(),
                                name: FIELD_ACTUAL.into(),
                            }
                        } else {
                            Expression::default_value_for_type(ty)
                        }
                        .into(),
                        false_expr: if has_return_value {
                            Expression::StructFieldAccess {
                                base: load.clone(),
                                name: FIELD_RETURNED.into(),
                            }
                        } else {
                            Expression::default_value_for_type(ty)
                        }
                        .into(),
                    },
                ])
            }
        }
    }

    fn into_return_object(self, ty: &Type, ret_ty: &Type) -> Expression {
        match self {
            ExpressionResult::Just(e) => make_struct(
                [
                    (FIELD_CONDITION, Type::Bool, Expression::BoolLiteral(true)),
                    (FIELD_ACTUAL, e.ty(), e),
                    (FIELD_RETURNED, ret_ty.clone(), Expression::default_value_for_type(ret_ty)),
                ]
                .into_iter(),
            ),
            ExpressionResult::MaybeReturn {
                pre_statements,
                condition,
                returned_value,
                actual_value,
            } => {
                let mut true_expr = ExpressionResult::Just(
                    actual_value.unwrap_or_else(|| Expression::default_value_for_type(ty)),
                )
                .into_return_object(ty, ret_ty);
                let mut false_expr =
                    ExpressionResult::Return(returned_value).into_return_object(ty, ret_ty);
                let true_ty = true_expr.ty();
                let false_ty = false_expr.ty();
                if true_ty != false_ty {
                    let common_ty = Expression::common_target_type_for_type_list(
                        [&true_ty, &false_ty].into_iter().cloned(),
                    );
                    if common_ty != true_ty {
                        true_expr =
                            convert_struct(std::mem::take(&mut true_expr), common_ty.clone())
                    }
                    if common_ty != false_ty {
                        false_expr = convert_struct(std::mem::take(&mut false_expr), common_ty)
                    }
                }
                let o = Expression::Condition {
                    condition: condition.into(),
                    true_expr: true_expr.into(),
                    false_expr: false_expr.into(),
                };
                codeblock_with_expr(pre_statements, o)
            }
            ExpressionResult::Return(r) => make_struct(
                [(FIELD_CONDITION, Type::Bool, Expression::BoolLiteral(false))]
                    .into_iter()
                    .chain(r.map(|r| (FIELD_RETURNED, ret_ty.clone(), r)))
                    .chain((!matches!(ty, Type::Void | Type::Invalid)).then(|| {
                        (FIELD_ACTUAL, ty.clone(), Expression::default_value_for_type(ty))
                    })),
            ),
            ExpressionResult::ReturnObject { value, .. } => value,
        }
    }

    fn map_value(self, f: impl FnOnce(Expression) -> Expression) -> Self {
        match self {
            ExpressionResult::Just(e) => ExpressionResult::Just(f(e)),
            ExpressionResult::Return(e) => ExpressionResult::Return(e),
            ExpressionResult::MaybeReturn {
                pre_statements,
                condition,
                returned_value,
                actual_value,
            } => ExpressionResult::MaybeReturn {
                pre_statements,
                condition,
                returned_value,
                actual_value: actual_value.map(f),
            },
            ExpressionResult::ReturnObject { value, has_value, has_return_value } => {
                if !has_value {
                    return ExpressionResult::ReturnObject { value, has_value, has_return_value };
                }
                static COUNT: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let name = format_smolstr!(
                    "mapped_expression{}",
                    COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                );
                let value_ty = value.ty();
                let load = |field: &str| Expression::StructFieldAccess {
                    base: Box::new(Expression::ReadLocalVariable {
                        name: name.clone(),
                        ty: value_ty.clone(),
                    }),
                    name: field.into(),
                };
                let condition = (FIELD_CONDITION, Type::Bool, load(FIELD_CONDITION));
                let actual = f(load(FIELD_ACTUAL));
                let actual = (FIELD_ACTUAL, actual.ty(), actual);
                let ret = has_return_value.then(|| {
                    let r = load(FIELD_RETURNED);
                    (FIELD_RETURNED, r.ty(), r)
                });
                ExpressionResult::ReturnObject {
                    value: Expression::CodeBlock(vec![
                        Expression::StoreLocalVariable { name, value: value.into() },
                        make_struct([condition, actual].into_iter().chain(ret.into_iter())),
                    ]),
                    has_value,
                    has_return_value,
                }
            }
        }
    }
}

fn codeblock_with_expr(mut pre_statements: Vec<Expression>, expr: Expression) -> Expression {
    if pre_statements.is_empty() {
        expr
    } else {
        pre_statements.push(expr);
        Expression::CodeBlock(pre_statements)
    }
}

fn make_struct(it: impl Iterator<Item = (&'static str, Type, Expression)>) -> Expression {
    let mut fields = BTreeMap::<SmolStr, Type>::new();
    let mut values = HashMap::<SmolStr, Expression>::new();
    let mut voids = Vec::new();
    for (name, ty, expr) in it {
        if matches!(ty, Type::Void | Type::Invalid) {
            if ty != Type::Invalid {
                voids.push(expr);
            }
            continue;
        }
        fields.insert(name.into(), ty);
        values.insert(name.into(), expr);
    }
    codeblock_with_expr(
        voids,
        Expression::Struct {
            ty: Type::Struct(Rc::new(Struct {
                fields,
                name: None,
                node: None,
                rust_attributes: None,
            })),
            values,
        },
    )
}

/// Given an expression `from` of type Struct, convert to another type struct with more fields
/// Add missing members in `from`
fn convert_struct(from: Expression, to: Type) -> Expression {
    let Type::Struct(s) = &to else {
        assert_eq!(to, Type::Invalid);
        return Expression::Invalid;
    };
    if let Expression::Struct { mut values, .. } = from {
        let mut new_values = HashMap::new();
        for (key, ty) in &s.fields {
            let (key, expression) = values
                .remove_entry(key)
                .unwrap_or_else(|| (key.clone(), Expression::default_value_for_type(ty)));
            new_values.insert(key, expression);
        }
        return Expression::Struct { values: new_values, ty: to };
    }
    static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let var_name = format_smolstr!(
        "tmpobj_ret_conv_{}",
        COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let from_ty = from.ty();
    let mut new_values = HashMap::new();
    let Type::Struct(from_s) = &from_ty else {
        assert_eq!(from_ty, Type::Invalid);
        return Expression::Invalid;
    };
    for (key, ty) in &s.fields {
        let expression = if from_s.fields.contains_key(key) {
            Expression::StructFieldAccess {
                base: Box::new(Expression::ReadLocalVariable {
                    name: var_name.clone(),
                    ty: from_ty.clone(),
                }),
                name: key.clone(),
            }
        } else {
            Expression::default_value_for_type(ty)
        };
        new_values.insert(key.clone(), expression);
    }
    Expression::CodeBlock(vec![
        Expression::StoreLocalVariable { name: var_name, value: Box::new(from) },
        Expression::Struct { values: new_values, ty: to },
    ])
}
