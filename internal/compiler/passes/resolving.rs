// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Passes that resolve the property binding expression.
//!
//! Before this pass, all the expression are of type Expression::Uncompiled,
//! and there should no longer be Uncompiled expression after this pass.
//!
//! Most of the code for the resolving actually lies in the expression_tree module

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::*;
use crate::langtype::{ElementType, Struct, Type};
use crate::lookup::{LookupCtx, LookupObject, LookupResult, LookupResultCallable};
use crate::object_tree::*;
use crate::parser::{identifier_text, syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode};
use crate::typeregister::TypeRegister;
use core::num::IntErrorKind;
use smol_str::{SmolStr, ToSmolStr};
use std::collections::HashMap;
use std::rc::Rc;

/// This represents a scope for the Component, where Component is the repeated component, but
/// does not represent a component in the .slint file
#[derive(Clone)]
struct ComponentScope(Vec<ElementRc>);

fn resolve_expression(
    elem: &ElementRc,
    expr: &mut Expression,
    property_name: Option<&str>,
    property_type: Type,
    scope: &[ElementRc],
    type_register: &TypeRegister,
    type_loader: &crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    if let Expression::Uncompiled(node) = expr.ignore_debug_hooks() {
        let mut lookup_ctx = LookupCtx {
            property_name,
            property_type,
            component_scope: scope,
            diag,
            arguments: vec![],
            type_register,
            type_loader: Some(type_loader),
            current_token: None,
            local_variables: vec![],
        };

        let new_expr = match node.kind() {
            SyntaxKind::CallbackConnection => {
                let node = syntax_nodes::CallbackConnection::from(node.clone());
                if let Some(property_name) = property_name {
                    check_callback_alias_validity(&node, elem, property_name, lookup_ctx.diag);
                }
                Expression::from_callback_connection(node, &mut lookup_ctx)
            }
            SyntaxKind::Function => Expression::from_function(node.clone().into(), &mut lookup_ctx),
            SyntaxKind::Expression => {
                //FIXME again: this happen for non-binding expression (i.e: model)
                Expression::from_expression_node(node.clone().into(), &mut lookup_ctx)
                    .maybe_convert_to(lookup_ctx.property_type.clone(), node, diag)
            }
            SyntaxKind::BindingExpression => {
                Expression::from_binding_expression_node(node.clone(), &mut lookup_ctx)
            }
            SyntaxKind::PropertyChangedCallback => Expression::from_codeblock_node(
                syntax_nodes::PropertyChangedCallback::from(node.clone()).CodeBlock(),
                &mut lookup_ctx,
            ),
            SyntaxKind::TwoWayBinding => {
                assert!(diag.has_errors(), "Two way binding should have been resolved already  (property: {property_name:?})");
                Expression::Invalid
            }
            _ => {
                debug_assert!(diag.has_errors());
                Expression::Invalid
            }
        };
        match expr {
            Expression::DebugHook { expression, .. } => *expression = Box::new(new_expr),
            _ => *expr = new_expr,
        }
    }
}

/// Call the visitor for each children of the element recursively, starting with the element itself
///
/// The item that is being visited will be pushed to the scope and popped once visitation is over.
fn recurse_elem_with_scope(
    elem: &ElementRc,
    mut scope: ComponentScope,
    vis: &mut impl FnMut(&ElementRc, &ComponentScope),
) -> ComponentScope {
    scope.0.push(elem.clone());
    vis(elem, &scope);
    for sub in &elem.borrow().children {
        scope = recurse_elem_with_scope(sub, scope, vis);
    }
    scope.0.pop();
    scope
}

pub fn resolve_expressions(
    doc: &Document,
    type_loader: &crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    resolve_two_way_bindings(doc, &doc.local_registry, diag);

    for component in doc.inner_components.iter() {
        recurse_elem_with_scope(
            &component.root_element,
            ComponentScope(vec![]),
            &mut |elem, scope| {
                let mut is_repeated = elem.borrow().repeated.is_some();
                visit_element_expressions(elem, |expr, property_name, property_type| {
                    let scope = if is_repeated {
                        // The first expression is always the model and it needs to be resolved with the parent scope
                        debug_assert!(matches!(
                            elem.borrow().repeated.as_ref().unwrap().model,
                            Expression::Invalid
                        )); // should be Invalid because it is taken by the visit_element_expressions function

                        is_repeated = false;

                        debug_assert!(scope.0.len() > 1);
                        &scope.0[..scope.0.len() - 1]
                    } else {
                        &scope.0
                    };

                    resolve_expression(
                        elem,
                        expr,
                        property_name,
                        property_type(),
                        scope,
                        &doc.local_registry,
                        type_loader,
                        diag,
                    );
                });
            },
        );
    }
}

/// To be used in [`Expression::from_qualified_name_node`] to specify if the lookup is performed
/// for two ways binding (which happens before the models and other expressions are resolved),
/// or after that.
#[derive(Default)]
enum LookupPhase {
    #[default]
    UnspecifiedPhase,
    ResolvingTwoWayBindings,
}

impl Expression {
    pub fn from_binding_expression_node(node: SyntaxNode, ctx: &mut LookupCtx) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::BindingExpression);
        let e = node
            .children()
            .find_map(|n| match n.kind() {
                SyntaxKind::Expression => Some(Self::from_expression_node(n.into(), ctx)),
                SyntaxKind::CodeBlock => Some(Self::from_codeblock_node(n.into(), ctx)),
                _ => None,
            })
            .unwrap_or(Self::Invalid);
        if ctx.property_type == Type::LogicalLength && e.ty() == Type::Percent {
            // See if a conversion from percentage to length is allowed
            const RELATIVE_TO_PARENT_PROPERTIES: &[&str] =
                &["width", "height", "preferred-width", "preferred-height"];
            let property_name = ctx.property_name.unwrap_or_default();
            if RELATIVE_TO_PARENT_PROPERTIES.contains(&property_name) {
                return e;
            } else {
                ctx.diag.push_error(
                    format!(
                        "Automatic conversion from percentage to length is only possible for the following properties: {}",
                        RELATIVE_TO_PARENT_PROPERTIES.join(", ")
                    ),
                    &node
                );
                return Expression::Invalid;
            }
        };
        if !matches!(ctx.property_type, Type::Callback { .. } | Type::Function { .. }) {
            e.maybe_convert_to(ctx.property_type.clone(), &node, ctx.diag)
        } else {
            // Binding to a callback or function shouldn't happen
            assert!(ctx.diag.has_errors());
            e
        }
    }

    fn from_codeblock_node(node: syntax_nodes::CodeBlock, ctx: &mut LookupCtx) -> Expression {
        debug_assert_eq!(node.kind(), SyntaxKind::CodeBlock);

        // new scope for locals
        ctx.local_variables.push(Vec::new());

        let mut statements_or_exprs = node
            .children()
            .filter_map(|n| match n.kind() {
                SyntaxKind::Expression => Some(Self::from_expression_node(n.into(), ctx)),
                SyntaxKind::ReturnStatement => Some(Self::from_return_statement(n.into(), ctx)),
                SyntaxKind::LetStatement => Some(Self::from_let_statement(n.into(), ctx)),
                _ => None,
            })
            .collect::<Vec<_>>();

        let exit_points_and_return_types = statements_or_exprs
            .iter()
            .enumerate()
            .filter_map(|(index, statement_or_expr)| {
                if index == statements_or_exprs.len()
                    || matches!(statement_or_expr, Expression::ReturnStatement(..))
                {
                    Some((index, statement_or_expr.ty()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let common_return_type = Self::common_target_type_for_type_list(
            exit_points_and_return_types.iter().map(|(_, ty)| ty.clone()),
        );

        exit_points_and_return_types.into_iter().for_each(|(index, _)| {
            let mut expr = std::mem::replace(&mut statements_or_exprs[index], Expression::Invalid);
            expr = expr.maybe_convert_to(common_return_type.clone(), &node, ctx.diag);
            statements_or_exprs[index] = expr;
        });

        // pop local scope
        ctx.local_variables.pop();

        Expression::CodeBlock(statements_or_exprs)
    }

    fn from_let_statement(node: syntax_nodes::LetStatement, ctx: &mut LookupCtx) -> Expression {
        let name = identifier_text(&node.DeclaredIdentifier()).unwrap_or_default();

        let global_lookup = crate::lookup::global_lookup();
        if let Some(LookupResult::Expression {
            expression:
                Expression::ReadLocalVariable { .. } | Expression::FunctionParameterReference { .. },
            ..
        }) = global_lookup.lookup(ctx, &name)
        {
            ctx.diag
                .push_error("Redeclaration of local variables is not allowed".to_string(), &node);
            return Expression::Invalid;
        }

        // prefix with "local_" to avoid conflicts
        let name: SmolStr = format!("local_{name}",).into();

        let value = Self::from_expression_node(node.Expression(), ctx);
        let ty = match node.Type() {
            Some(ty) => type_from_node(ty, ctx.diag, ctx.type_register),
            None => value.ty(),
        };

        // we can get the last scope exists, because each codeblock creates a new scope and we are inside a codeblock here by necessity
        ctx.local_variables.last_mut().unwrap().push((name.clone(), ty.clone()));

        let value = Box::new(value.maybe_convert_to(ty.clone(), &node, ctx.diag));

        Expression::StoreLocalVariable { name, value }
    }

    fn from_return_statement(
        node: syntax_nodes::ReturnStatement,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let return_type = ctx.return_type().clone();
        let e = node.Expression();
        if e.is_none() && !matches!(return_type, Type::Void | Type::Invalid) {
            ctx.diag.push_error(format!("Must return a value of type '{return_type}'"), &node);
        }
        Expression::ReturnStatement(e.map(|n| {
            Box::new(Self::from_expression_node(n, ctx).maybe_convert_to(
                return_type,
                &node,
                ctx.diag,
            ))
        }))
    }

    fn from_callback_connection(
        node: syntax_nodes::CallbackConnection,
        ctx: &mut LookupCtx,
    ) -> Expression {
        ctx.arguments =
            node.DeclaredIdentifier().map(|x| identifier_text(&x).unwrap_or_default()).collect();
        if let Some(code_block_node) = node.CodeBlock() {
            Self::from_codeblock_node(code_block_node, ctx).maybe_convert_to(
                ctx.return_type().clone(),
                &node,
                ctx.diag,
            )
        } else if let Some(expr_node) = node.Expression() {
            Self::from_expression_node(expr_node, ctx).maybe_convert_to(
                ctx.return_type().clone(),
                &node,
                ctx.diag,
            )
        } else {
            return Expression::Invalid;
        }
    }

    fn from_function(node: syntax_nodes::Function, ctx: &mut LookupCtx) -> Expression {
        ctx.arguments = node
            .ArgumentDeclaration()
            .map(|x| identifier_text(&x.DeclaredIdentifier()).unwrap_or_default())
            .collect();
        Self::from_codeblock_node(node.CodeBlock(), ctx).maybe_convert_to(
            ctx.return_type().clone(),
            &node,
            ctx.diag,
        )
    }

    pub fn from_expression_node(node: syntax_nodes::Expression, ctx: &mut LookupCtx) -> Self {
        node.children_with_tokens()
            .find_map(|child| match child {
                NodeOrToken::Node(node) => match node.kind() {
                    SyntaxKind::Expression => Some(Self::from_expression_node(node.into(), ctx)),
                    SyntaxKind::AtImageUrl => Some(Self::from_at_image_url_node(node.into(), ctx)),
                    SyntaxKind::AtGradient => Some(Self::from_at_gradient(node.into(), ctx)),
                    SyntaxKind::AtTr => Some(Self::from_at_tr(node.into(), ctx)),
                    SyntaxKind::QualifiedName => Some(Self::from_qualified_name_node(
                        node.clone().into(),
                        ctx,
                        LookupPhase::default(),
                    )),
                    SyntaxKind::FunctionCallExpression => {
                        Some(Self::from_function_call_node(node.into(), ctx))
                    }
                    SyntaxKind::MemberAccess => {
                        Some(Self::from_member_access_node(node.into(), ctx))
                    }
                    SyntaxKind::IndexExpression => {
                        Some(Self::from_index_expression_node(node.into(), ctx))
                    }
                    SyntaxKind::SelfAssignment => {
                        Some(Self::from_self_assignment_node(node.into(), ctx))
                    }
                    SyntaxKind::BinaryExpression => {
                        Some(Self::from_binary_expression_node(node.into(), ctx))
                    }
                    SyntaxKind::UnaryOpExpression => {
                        Some(Self::from_unaryop_expression_node(node.into(), ctx))
                    }
                    SyntaxKind::ConditionalExpression => {
                        Some(Self::from_conditional_expression_node(node.into(), ctx))
                    }
                    SyntaxKind::ObjectLiteral => {
                        Some(Self::from_object_literal_node(node.into(), ctx))
                    }
                    SyntaxKind::Array => Some(Self::from_array_node(node.into(), ctx)),
                    SyntaxKind::CodeBlock => Some(Self::from_codeblock_node(node.into(), ctx)),
                    SyntaxKind::StringTemplate => {
                        Some(Self::from_string_template_node(node.into(), ctx))
                    }
                    _ => None,
                },
                NodeOrToken::Token(token) => match token.kind() {
                    SyntaxKind::StringLiteral => Some(
                        crate::literals::unescape_string(token.text())
                            .map(Self::StringLiteral)
                            .unwrap_or_else(|| {
                                ctx.diag.push_error("Cannot parse string literal".into(), &token);
                                Self::Invalid
                            }),
                    ),
                    SyntaxKind::NumberLiteral => Some(
                        crate::literals::parse_number_literal(token.text().into()).unwrap_or_else(
                            |e| {
                                ctx.diag.push_error(e.to_string(), &node);
                                Self::Invalid
                            },
                        ),
                    ),
                    SyntaxKind::ColorLiteral => Some(
                        crate::literals::parse_color_literal(token.text())
                            .map(|i| Expression::Cast {
                                from: Box::new(Expression::NumberLiteral(i as _, Unit::None)),
                                to: Type::Color,
                            })
                            .unwrap_or_else(|| {
                                ctx.diag.push_error("Invalid color literal".into(), &node);
                                Self::Invalid
                            }),
                    ),

                    _ => None,
                },
            })
            .unwrap_or(Self::Invalid)
    }

    fn from_at_image_url_node(node: syntax_nodes::AtImageUrl, ctx: &mut LookupCtx) -> Self {
        let s = match node
            .child_text(SyntaxKind::StringLiteral)
            .and_then(|x| crate::literals::unescape_string(&x))
        {
            Some(s) => s,
            None => {
                ctx.diag.push_error("Cannot parse string literal".into(), &node);
                return Self::Invalid;
            }
        };

        if s.is_empty() {
            return Expression::ImageReference {
                resource_ref: ImageReference::None,
                source_location: Some(node.to_source_location()),
                nine_slice: None,
            };
        }

        let absolute_source_path = {
            let path = std::path::Path::new(&s);
            if crate::pathutils::is_absolute(path) {
                s
            } else {
                ctx.type_loader
                    .and_then(|loader| {
                        loader.resolve_import_path(Some(&(*node).clone().into()), &s)
                    })
                    .map(|i| i.0.to_string_lossy().into())
                    .unwrap_or_else(|| {
                        crate::pathutils::join(
                            &crate::pathutils::dirname(node.source_file.path()),
                            path,
                        )
                        .map(|p| p.to_string_lossy().into())
                        .unwrap_or(s.clone())
                    })
            }
        };

        let nine_slice = node
            .children_with_tokens()
            .filter_map(|n| n.into_token())
            .filter(|t| t.kind() == SyntaxKind::NumberLiteral)
            .map(|arg| {
                arg.text().parse().unwrap_or_else(|err: std::num::ParseIntError| {
                    match err.kind() {
                        IntErrorKind::PosOverflow | IntErrorKind::NegOverflow => {
                            ctx.diag.push_error("Number too big".into(), &arg)
                        }
                        IntErrorKind::InvalidDigit => ctx.diag.push_error(
                            "Border widths of a nine-slice can't have units".into(),
                            &arg,
                        ),
                        _ => ctx.diag.push_error("Cannot parse number literal".into(), &arg),
                    };
                    0u16
                })
            })
            .collect::<Vec<u16>>();

        let nine_slice = match nine_slice.as_slice() {
            [x] => Some([*x, *x, *x, *x]),
            [x, y] => Some([*x, *y, *x, *y]),
            [x, y, z, w] => Some([*x, *y, *z, *w]),
            [] => None,
            _ => {
                assert!(ctx.diag.has_errors());
                None
            }
        };

        Expression::ImageReference {
            resource_ref: ImageReference::AbsolutePath(absolute_source_path),
            source_location: Some(node.to_source_location()),
            nine_slice,
        }
    }

    pub fn from_at_gradient(node: syntax_nodes::AtGradient, ctx: &mut LookupCtx) -> Self {
        enum GradKind {
            Linear { angle: Box<Expression> },
            Radial,
            Conic,
        }

        let mut subs = node
            .children_with_tokens()
            .filter(|n| matches!(n.kind(), SyntaxKind::Comma | SyntaxKind::Expression));

        let grad_token = node.child_token(SyntaxKind::Identifier).unwrap();
        let grad_text = grad_token.text();

        let grad_kind = if grad_text.starts_with("linear") {
            let angle_expr = match subs.next() {
                Some(e) if e.kind() == SyntaxKind::Expression => {
                    syntax_nodes::Expression::from(e.into_node().unwrap())
                }
                _ => {
                    ctx.diag.push_error("Expected angle expression".into(), &node);
                    return Expression::Invalid;
                }
            };
            if subs.next().is_some_and(|s| s.kind() != SyntaxKind::Comma) {
                ctx.diag.push_error(
                    "Angle expression must be an angle followed by a comma".into(),
                    &node,
                );
                return Expression::Invalid;
            }
            let angle = Box::new(
                Expression::from_expression_node(angle_expr.clone(), ctx).maybe_convert_to(
                    Type::Angle,
                    &angle_expr,
                    ctx.diag,
                ),
            );
            GradKind::Linear { angle }
        } else if grad_text.starts_with("radial") {
            if !matches!(subs.next(), Some(NodeOrToken::Node(n)) if n.text().to_string().trim() == "circle")
            {
                ctx.diag.push_error("Expected 'circle': currently, only @radial-gradient(circle, ...) are supported".into(), &node);
                return Expression::Invalid;
            }
            let comma = subs.next();
            if matches!(&comma, Some(NodeOrToken::Node(n)) if n.text().to_string().trim() == "at") {
                ctx.diag.push_error("'at' in @radial-gradient is not yet supported".into(), &comma);
                return Expression::Invalid;
            }
            if comma.as_ref().is_some_and(|s| s.kind() != SyntaxKind::Comma) {
                ctx.diag.push_error(
                    "'circle' must be followed by a comma".into(),
                    comma.as_ref().map_or(&node, |x| x as &dyn Spanned),
                );
                return Expression::Invalid;
            }
            GradKind::Radial
        } else if grad_text.starts_with("conic") {
            GradKind::Conic
        } else {
            // Parser should have ensured we have one of the linear, radial or conic gradient
            panic!("Not a gradient {grad_text:?}");
        };

        let mut stops = vec![];
        enum Stop {
            Empty,
            Color(Expression),
            Finished,
        }
        let mut current_stop = Stop::Empty;
        for n in subs {
            if n.kind() == SyntaxKind::Comma {
                match std::mem::replace(&mut current_stop, Stop::Empty) {
                    Stop::Empty => {
                        ctx.diag.push_error("Expected expression".into(), &n);
                        break;
                    }
                    Stop::Finished => {}
                    Stop::Color(col) => stops.push((
                        col,
                        if stops.is_empty() {
                            Expression::NumberLiteral(0., Unit::None)
                        } else {
                            Expression::Invalid
                        },
                    )),
                }
            } else {
                // To facilitate color literal conversion, adjust the expected return type.
                let e = {
                    let old_property_type = std::mem::replace(&mut ctx.property_type, Type::Color);
                    let e =
                        Expression::from_expression_node(n.as_node().unwrap().clone().into(), ctx);
                    ctx.property_type = old_property_type;
                    e
                };
                match std::mem::replace(&mut current_stop, Stop::Finished) {
                    Stop::Empty => {
                        current_stop = Stop::Color(e.maybe_convert_to(Type::Color, &n, ctx.diag))
                    }
                    Stop::Finished => {
                        ctx.diag.push_error("Expected comma".into(), &n);
                        break;
                    }
                    Stop::Color(col) => {
                        let stop_type = match &grad_kind {
                            GradKind::Conic => Type::Angle,
                            _ => Type::Float32,
                        };
                        stops.push((col, e.maybe_convert_to(stop_type, &n, ctx.diag)))
                    }
                }
            }
        }
        match current_stop {
            Stop::Color(col) => stops.push((col, Expression::NumberLiteral(1., Unit::None))),
            Stop::Empty => {
                if let Some((_, e @ Expression::Invalid)) = stops.last_mut() {
                    *e = Expression::NumberLiteral(1., Unit::None)
                }
            }
            Stop::Finished => (),
        };

        // Fix the stop so each has a position.
        let mut start = 0;
        while start < stops.len() {
            start += match stops[start..].iter().position(|s| matches!(s.1, Expression::Invalid)) {
                Some(p) => p,
                None => break,
            };
            let (before, rest) = stops.split_at_mut(start);
            let pos =
                rest.iter().position(|s| !matches!(s.1, Expression::Invalid)).unwrap_or(rest.len());
            if pos > 0 && pos < rest.len() {
                let (middle, after) = rest.split_at_mut(pos);
                let begin = before
                    .last()
                    .map(|s| &s.1)
                    .unwrap_or(&Expression::NumberLiteral(1., Unit::None));
                let end = &after.first().expect("The last should never be invalid").1;
                for (i, (_, e)) in middle.iter_mut().enumerate() {
                    debug_assert!(matches!(e, Expression::Invalid));
                    // e = begin + (i+1) * (end - begin) / (pos+1)
                    *e = Expression::BinaryExpression {
                        lhs: Box::new(begin.clone()),
                        rhs: Box::new(Expression::BinaryExpression {
                            lhs: Box::new(Expression::BinaryExpression {
                                lhs: Box::new(Expression::NumberLiteral(i as f64 + 1., Unit::None)),
                                rhs: Box::new(Expression::BinaryExpression {
                                    lhs: Box::new(end.clone()),
                                    rhs: Box::new(begin.clone()),
                                    op: '-',
                                }),
                                op: '*',
                            }),
                            rhs: Box::new(Expression::NumberLiteral(pos as f64 + 1., Unit::None)),
                            op: '/',
                        }),
                        op: '+',
                    };
                }
            }
            start += pos + 1;
        }

        match grad_kind {
            GradKind::Linear { angle } => Expression::LinearGradient { angle, stops },
            GradKind::Radial => Expression::RadialGradient { stops },
            GradKind::Conic => {
                // For conic gradients, we need to:
                // 1. Ensure angle expressions are converted to Type::Angle
                // 2. Normalize to 0-1 range for internal representation
                let normalized_stops = stops
                    .into_iter()
                    .map(|(color, angle_expr)| {
                        // First ensure the angle expression is properly typed as Angle
                        let angle_typed =
                            angle_expr.maybe_convert_to(Type::Angle, &node, &mut ctx.diag);

                        // Convert angle to 0-1 range by dividing by 360deg
                        // This ensures all angle units (deg, rad, turn) are normalized
                        let normalized_pos = Expression::BinaryExpression {
                            lhs: Box::new(angle_typed),
                            rhs: Box::new(Expression::NumberLiteral(360., Unit::Deg)),
                            op: '/',
                        };
                        (color, normalized_pos)
                    })
                    .collect();
                Expression::ConicGradient { stops: normalized_stops }
            }
        }
    }

    fn from_at_tr(node: syntax_nodes::AtTr, ctx: &mut LookupCtx) -> Expression {
        let Some(string) = node
            .child_text(SyntaxKind::StringLiteral)
            .and_then(|s| crate::literals::unescape_string(&s))
        else {
            ctx.diag.push_error("Cannot parse string literal".into(), &node);
            return Expression::Invalid;
        };
        let context = node.TrContext().map(|n| {
            n.child_text(SyntaxKind::StringLiteral)
                .and_then(|s| crate::literals::unescape_string(&s))
                .unwrap_or_else(|| {
                    ctx.diag.push_error("Cannot parse string literal".into(), &n);
                    Default::default()
                })
        });
        let plural = node.TrPlural().map(|pl| {
            let s = pl
                .child_text(SyntaxKind::StringLiteral)
                .and_then(|s| crate::literals::unescape_string(&s))
                .unwrap_or_else(|| {
                    ctx.diag.push_error("Cannot parse string literal".into(), &pl);
                    Default::default()
                });
            let n = pl.Expression();
            let expr = Expression::from_expression_node(n.clone(), ctx).maybe_convert_to(
                Type::Int32,
                &n,
                ctx.diag,
            );
            (s, expr)
        });

        let domain = ctx
            .type_loader
            .and_then(|tl| tl.compiler_config.translation_domain.clone())
            .unwrap_or_default();

        let subs = node.Expression().map(|n| {
            Expression::from_expression_node(n.clone(), ctx).maybe_convert_to(
                Type::String,
                &n,
                ctx.diag,
            )
        });
        let values = subs.collect::<Vec<_>>();

        // check format string
        {
            let mut arg_idx = 0;
            let mut pos_max = 0;
            let mut pos = 0;
            let mut has_n = false;
            while let Some(mut p) = string[pos..].find(['{', '}']) {
                if string.len() - pos < p + 1 {
                    ctx.diag.push_error(
                        "Unescaped trailing '{' in format string. Escape '{' with '{{'".into(),
                        &node,
                    );
                    break;
                }
                p += pos;

                // Skip escaped }
                if string.get(p..=p) == Some("}") {
                    if string.get(p + 1..=p + 1) == Some("}") {
                        pos = p + 2;
                        continue;
                    } else {
                        ctx.diag.push_error(
                            "Unescaped '}' in format string. Escape '}' with '}}'".into(),
                            &node,
                        );
                        break;
                    }
                }

                // Skip escaped {
                if string.get(p + 1..=p + 1) == Some("{") {
                    pos = p + 2;
                    continue;
                }

                // Find the argument
                let end = if let Some(end) = string[p..].find('}') {
                    end + p
                } else {
                    ctx.diag.push_error(
                        "Unterminated placeholder in format string. '{' must be escaped with '{{'"
                            .into(),
                        &node,
                    );
                    break;
                };
                let argument = &string[p + 1..end];
                if argument.is_empty() {
                    arg_idx += 1;
                } else if let Ok(n) = argument.parse::<u16>() {
                    pos_max = pos_max.max(n as usize + 1);
                } else if argument == "n" {
                    has_n = true;
                    if plural.is_none() {
                        ctx.diag.push_error(
                            "`{n}` placeholder can only be found in plural form".into(),
                            &node,
                        );
                    }
                } else {
                    ctx.diag
                        .push_error("Invalid '{...}' placeholder in format string. The placeholder must be a number, or braces must be escaped with '{{' and '}}'".into(), &node);
                    break;
                };
                pos = end + 1;
            }
            if arg_idx > 0 && pos_max > 0 {
                ctx.diag.push_error(
                    "Cannot mix positional and non-positional placeholder in format string".into(),
                    &node,
                );
            } else if arg_idx > values.len() || pos_max > values.len() {
                let num = arg_idx.max(pos_max);
                let note = if !has_n && plural.is_some() {
                    ". Note: use `{n}` for the argument after '%'"
                } else {
                    ""
                };
                ctx.diag.push_error(
                    format!("Format string contains {num} placeholders, but only {} extra arguments were given{note}", values.len()),
                    &node,
                );
            }
        }

        let plural =
            plural.unwrap_or((SmolStr::default(), Expression::NumberLiteral(1., Unit::None)));

        let get_component_name = || {
            ctx.component_scope
                .first()
                .and_then(|e| e.borrow().enclosing_component.upgrade())
                .map(|c| c.id.clone())
        };

        Expression::FunctionCall {
            function: BuiltinFunction::Translate.into(),
            arguments: vec![
                Expression::StringLiteral(string),
                Expression::StringLiteral(context.or_else(get_component_name).unwrap_or_default()),
                Expression::StringLiteral(domain.into()),
                Expression::Array { element_ty: Type::String, values },
                plural.1,
                Expression::StringLiteral(plural.0),
            ],
            source_location: Some(node.to_source_location()),
        }
    }

    /// Perform the lookup
    fn from_qualified_name_node(
        node: syntax_nodes::QualifiedName,
        ctx: &mut LookupCtx,
        phase: LookupPhase,
    ) -> Self {
        Self::from_lookup_result(lookup_qualified_name_node(node.clone(), ctx, phase), ctx, &node)
    }

    fn from_lookup_result(
        r: Option<LookupResult>,
        ctx: &mut LookupCtx,
        node: &dyn Spanned,
    ) -> Self {
        let Some(r) = r else {
            assert!(ctx.diag.has_errors());
            return Self::Invalid;
        };
        match r {
            LookupResult::Expression { expression, .. } => expression,
            LookupResult::Callable(c) => {
                let what = match c {
                    LookupResultCallable::Callable(Callable::Callback(..)) => "Callback",
                    LookupResultCallable::Callable(Callable::Builtin(..)) => "Builtin function",
                    LookupResultCallable::Macro(..) => "Builtin function",
                    LookupResultCallable::MemberFunction { .. } => "Member function",
                    _ => "Function",
                };
                ctx.diag
                    .push_error(format!("{what} must be called. Did you forgot the '()'?",), node);
                Self::Invalid
            }
            LookupResult::Enumeration(..) => {
                ctx.diag.push_error("Cannot take reference to an enum".to_string(), node);
                Self::Invalid
            }
            LookupResult::Namespace(..) => {
                ctx.diag.push_error("Cannot take reference to a namespace".to_string(), node);
                Self::Invalid
            }
        }
    }

    fn from_function_call_node(
        node: syntax_nodes::FunctionCallExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let mut arguments = Vec::new();

        let mut sub_expr = node.Expression();

        let func_expr = sub_expr.next().unwrap();

        let (function, source_location) = if let Some(qn) = func_expr.QualifiedName() {
            let sl = qn.last_token().unwrap().to_source_location();
            (lookup_qualified_name_node(qn, ctx, LookupPhase::default()), sl)
        } else if let Some(ma) = func_expr.MemberAccess() {
            let base = Self::from_expression_node(ma.Expression(), ctx);
            let field = ma.child_token(SyntaxKind::Identifier);
            let sl = field.to_source_location();
            (maybe_lookup_object(base.into(), field.clone().into_iter(), ctx), sl)
        } else {
            if Self::from_expression_node(func_expr, ctx).ty() == Type::Invalid {
                assert!(ctx.diag.has_errors());
            } else {
                ctx.diag.push_error("The expression is not a function".into(), &node);
            }
            return Self::Invalid;
        };
        let sub_expr = sub_expr.map(|n| {
            (Self::from_expression_node(n.clone(), ctx), Some(NodeOrToken::from((*n).clone())))
        });
        let Some(function) = function else {
            // Check sub expressions anyway
            sub_expr.count();
            assert!(ctx.diag.has_errors());
            return Self::Invalid;
        };
        let LookupResult::Callable(function) = function else {
            // Check sub expressions anyway
            sub_expr.count();
            ctx.diag.push_error("The expression is not a function".into(), &node);
            return Self::Invalid;
        };

        let mut adjust_arg_count = 0;
        let function = match function {
            LookupResultCallable::Callable(c) => c,
            LookupResultCallable::Macro(mac) => {
                arguments.extend(sub_expr);
                return crate::builtin_macros::lower_macro(
                    mac,
                    &source_location,
                    arguments.into_iter(),
                    ctx.diag,
                );
            }
            LookupResultCallable::MemberFunction { member, base, base_node } => {
                arguments.push((base, base_node));
                adjust_arg_count = 1;
                match *member {
                    LookupResultCallable::Callable(c) => c,
                    LookupResultCallable::Macro(mac) => {
                        arguments.extend(sub_expr);
                        return crate::builtin_macros::lower_macro(
                            mac,
                            &source_location,
                            arguments.into_iter(),
                            ctx.diag,
                        );
                    }
                    LookupResultCallable::MemberFunction { .. } => {
                        unreachable!()
                    }
                }
            }
        };

        arguments.extend(sub_expr);

        let arguments = match function.ty() {
            Type::Function(function) | Type::Callback(function) => {
                if arguments.len() != function.args.len() {
                    ctx.diag.push_error(
                        format!(
                            "The callback or function expects {} arguments, but {} are provided",
                            function.args.len() - adjust_arg_count,
                            arguments.len() - adjust_arg_count,
                        ),
                        &node,
                    );
                    arguments.into_iter().map(|x| x.0).collect()
                } else {
                    arguments
                        .into_iter()
                        .zip(function.args.iter())
                        .map(|((e, node), ty)| e.maybe_convert_to(ty.clone(), &node, ctx.diag))
                        .collect()
                }
            }
            Type::Invalid => {
                debug_assert!(ctx.diag.has_errors(), "The error must already have been reported.");
                arguments.into_iter().map(|x| x.0).collect()
            }
            _ => {
                ctx.diag.push_error("The expression is not a function".into(), &node);
                arguments.into_iter().map(|x| x.0).collect()
            }
        };

        Expression::FunctionCall { function, arguments, source_location: Some(source_location) }
    }

    fn from_member_access_node(
        node: syntax_nodes::MemberAccess,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let base = Self::from_expression_node(node.Expression(), ctx);
        let field = node.child_token(SyntaxKind::Identifier);
        Self::from_lookup_result(
            maybe_lookup_object(base.into(), field.clone().into_iter(), ctx),
            ctx,
            &field,
        )
    }

    fn from_self_assignment_node(
        node: syntax_nodes::SelfAssignment,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let (lhs_n, rhs_n) = node.Expression();
        let mut lhs = Self::from_expression_node(lhs_n.clone(), ctx);
        let op = node
            .children_with_tokens()
            .find_map(|n| match n.kind() {
                SyntaxKind::PlusEqual => Some('+'),
                SyntaxKind::MinusEqual => Some('-'),
                SyntaxKind::StarEqual => Some('*'),
                SyntaxKind::DivEqual => Some('/'),
                SyntaxKind::Equal => Some('='),
                _ => None,
            })
            .unwrap_or('_');
        if lhs.ty() != Type::Invalid {
            lhs.try_set_rw(ctx, if op == '=' { "Assignment" } else { "Self assignment" }, &node);
        }
        let ty = lhs.ty();
        let expected_ty = match op {
            '=' => ty,
            '+' if ty == Type::String || ty.as_unit_product().is_some() => ty,
            '-' if ty.as_unit_product().is_some() => ty,
            '/' | '*' if ty.as_unit_product().is_some() => Type::Float32,
            _ => {
                if ty != Type::Invalid {
                    ctx.diag.push_error(
                        format!("the {op}= operation cannot be done on a {ty}"),
                        &lhs_n,
                    );
                }
                Type::Invalid
            }
        };
        let rhs = Self::from_expression_node(rhs_n.clone(), ctx);
        Expression::SelfAssignment {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs.maybe_convert_to(expected_ty, &rhs_n, ctx.diag)),
            op,
            node: Some(NodeOrToken::Node(node.into())),
        }
    }

    fn from_binary_expression_node(
        node: syntax_nodes::BinaryExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let op = node
            .children_with_tokens()
            .find_map(|n| match n.kind() {
                SyntaxKind::Plus => Some('+'),
                SyntaxKind::Minus => Some('-'),
                SyntaxKind::Star => Some('*'),
                SyntaxKind::Div => Some('/'),
                SyntaxKind::LessEqual => Some('≤'),
                SyntaxKind::GreaterEqual => Some('≥'),
                SyntaxKind::LAngle => Some('<'),
                SyntaxKind::RAngle => Some('>'),
                SyntaxKind::EqualEqual => Some('='),
                SyntaxKind::NotEqual => Some('!'),
                SyntaxKind::AndAnd => Some('&'),
                SyntaxKind::OrOr => Some('|'),
                _ => None,
            })
            .unwrap_or('_');

        let (lhs_n, rhs_n) = node.Expression();
        let lhs = Self::from_expression_node(lhs_n.clone(), ctx);
        let rhs = Self::from_expression_node(rhs_n.clone(), ctx);

        let expected_ty = match operator_class(op) {
            OperatorClass::ComparisonOp => {
                let ty =
                    Self::common_target_type_for_type_list([lhs.ty(), rhs.ty()].iter().cloned());
                if !matches!(op, '=' | '!') && !ty.as_unit_product().is_some() && ty != Type::String
                {
                    ctx.diag.push_error(format!("Values of type {ty} cannot be compared"), &node);
                }
                ty
            }
            OperatorClass::LogicalOp => Type::Bool,
            OperatorClass::ArithmeticOp => {
                let (lhs_ty, rhs_ty) = (lhs.ty(), rhs.ty());
                if op == '+' && (lhs_ty == Type::String || rhs_ty == Type::String) {
                    Type::String
                } else if op == '+' || op == '-' {
                    if lhs_ty.default_unit().is_some() {
                        lhs_ty
                    } else if rhs_ty.default_unit().is_some() {
                        rhs_ty
                    } else if matches!(lhs_ty, Type::UnitProduct(_)) {
                        lhs_ty
                    } else if matches!(rhs_ty, Type::UnitProduct(_)) {
                        rhs_ty
                    } else {
                        Type::Float32
                    }
                } else if op == '*' || op == '/' {
                    let has_unit = |ty: &Type| {
                        matches!(ty, Type::UnitProduct(_)) || ty.default_unit().is_some()
                    };
                    match (has_unit(&lhs_ty), has_unit(&rhs_ty)) {
                        (true, true) => {
                            return Expression::BinaryExpression {
                                lhs: Box::new(lhs),
                                rhs: Box::new(rhs),
                                op,
                            }
                        }
                        (true, false) => {
                            return Expression::BinaryExpression {
                                lhs: Box::new(lhs),
                                rhs: Box::new(rhs.maybe_convert_to(
                                    Type::Float32,
                                    &rhs_n,
                                    ctx.diag,
                                )),
                                op,
                            }
                        }
                        (false, true) => {
                            return Expression::BinaryExpression {
                                lhs: Box::new(lhs.maybe_convert_to(
                                    Type::Float32,
                                    &lhs_n,
                                    ctx.diag,
                                )),
                                rhs: Box::new(rhs),
                                op,
                            }
                        }
                        (false, false) => Type::Float32,
                    }
                } else {
                    unreachable!()
                }
            }
        };
        Expression::BinaryExpression {
            lhs: Box::new(lhs.maybe_convert_to(expected_ty.clone(), &lhs_n, ctx.diag)),
            rhs: Box::new(rhs.maybe_convert_to(expected_ty, &rhs_n, ctx.diag)),
            op,
        }
    }

    fn from_unaryop_expression_node(
        node: syntax_nodes::UnaryOpExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let exp_n = node.Expression();
        let exp = Self::from_expression_node(exp_n, ctx);

        let op = node
            .children_with_tokens()
            .find_map(|n| match n.kind() {
                SyntaxKind::Plus => Some('+'),
                SyntaxKind::Minus => Some('-'),
                SyntaxKind::Bang => Some('!'),
                _ => None,
            })
            .unwrap_or('_');

        let exp = match op {
            '!' => exp.maybe_convert_to(Type::Bool, &node, ctx.diag),
            '+' | '-' => {
                let ty = exp.ty();
                if ty.default_unit().is_none()
                    && !matches!(
                        ty,
                        Type::Int32
                            | Type::Float32
                            | Type::Percent
                            | Type::UnitProduct(..)
                            | Type::Invalid
                    )
                {
                    ctx.diag.push_error(format!("Unary '{op}' not supported on {ty}"), &node);
                }
                exp
            }
            _ => {
                assert!(ctx.diag.has_errors());
                exp
            }
        };

        Expression::UnaryOp { sub: Box::new(exp), op }
    }

    fn from_conditional_expression_node(
        node: syntax_nodes::ConditionalExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let (condition_n, true_expr_n, false_expr_n) = node.Expression();
        // FIXME: we should we add bool to the context
        let condition = Self::from_expression_node(condition_n.clone(), ctx).maybe_convert_to(
            Type::Bool,
            &condition_n,
            ctx.diag,
        );
        let true_expr = Self::from_expression_node(true_expr_n.clone(), ctx);
        let false_expr = Self::from_expression_node(false_expr_n.clone(), ctx);
        let result_ty = Self::common_target_type_for_type_list(
            [true_expr.ty(), false_expr.ty()].iter().cloned(),
        );
        let true_expr = true_expr.maybe_convert_to(result_ty.clone(), &true_expr_n, ctx.diag);
        let false_expr = false_expr.maybe_convert_to(result_ty, &false_expr_n, ctx.diag);
        Expression::Condition {
            condition: Box::new(condition),
            true_expr: Box::new(true_expr),
            false_expr: Box::new(false_expr),
        }
    }

    fn from_index_expression_node(
        node: syntax_nodes::IndexExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let (array_expr_n, index_expr_n) = node.Expression();
        let array_expr = Self::from_expression_node(array_expr_n, ctx);
        let index_expr = Self::from_expression_node(index_expr_n.clone(), ctx).maybe_convert_to(
            Type::Int32,
            &index_expr_n,
            ctx.diag,
        );

        let ty = array_expr.ty();
        if !matches!(ty, Type::Array(_) | Type::Invalid | Type::Function(_) | Type::Callback(_)) {
            ctx.diag.push_error(format!("{ty} is not an indexable type"), &node);
        }
        Expression::ArrayIndex { array: Box::new(array_expr), index: Box::new(index_expr) }
    }

    fn from_object_literal_node(
        node: syntax_nodes::ObjectLiteral,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let values: HashMap<SmolStr, Expression> = node
            .ObjectMember()
            .map(|n| {
                (
                    identifier_text(&n).unwrap_or_default(),
                    Expression::from_expression_node(n.Expression(), ctx),
                )
            })
            .collect();
        let ty = Rc::new(Struct {
            fields: values.iter().map(|(k, v)| (k.clone(), v.ty())).collect(),
            name: None,
            node: None,
            rust_attributes: None,
        });
        Expression::Struct { ty, values }
    }

    fn from_array_node(node: syntax_nodes::Array, ctx: &mut LookupCtx) -> Expression {
        let mut values: Vec<Expression> =
            node.Expression().map(|e| Expression::from_expression_node(e, ctx)).collect();

        let element_ty = if values.is_empty() {
            Type::Void
        } else {
            Self::common_target_type_for_type_list(values.iter().map(|expr| expr.ty()))
        };

        for e in values.iter_mut() {
            *e = core::mem::replace(e, Expression::Invalid).maybe_convert_to(
                element_ty.clone(),
                &node,
                ctx.diag,
            );
        }

        Expression::Array { element_ty, values }
    }

    fn from_string_template_node(
        node: syntax_nodes::StringTemplate,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let mut exprs = node.Expression().map(|e| {
            Expression::from_expression_node(e.clone(), ctx).maybe_convert_to(
                Type::String,
                &e,
                ctx.diag,
            )
        });
        let mut result = exprs.next().unwrap_or_default();
        for x in exprs {
            result = Expression::BinaryExpression {
                lhs: Box::new(std::mem::take(&mut result)),
                rhs: Box::new(x),
                op: '+',
            }
        }
        result
    }

    /// This function is used to find a type that's suitable for casting each instance of a bunch of expressions
    /// to a type that captures most aspects. For example for an array of object literals the result is a merge of
    /// all seen fields.
    pub fn common_target_type_for_type_list(types: impl Iterator<Item = Type>) -> Type {
        types.fold(Type::Invalid, |target_type, expr_ty| {
            if target_type == expr_ty {
                target_type
            } else if target_type == Type::Invalid {
                expr_ty
            } else {
                match (target_type, expr_ty) {
                    (Type::Struct(ref result), Type::Struct(ref elem)) => {
                        let mut fields = result.fields.clone();
                        for (elem_name, elem_ty) in elem.fields.iter() {
                            match fields.entry(elem_name.clone()) {
                                std::collections::btree_map::Entry::Vacant(free_entry) => {
                                    free_entry.insert(elem_ty.clone());
                                }
                                std::collections::btree_map::Entry::Occupied(
                                    mut existing_field,
                                ) => {
                                    *existing_field.get_mut() =
                                        Self::common_target_type_for_type_list(
                                            [existing_field.get().clone(), elem_ty.clone()]
                                                .into_iter(),
                                        );
                                }
                            }
                        }
                        Type::Struct(Rc::new(Struct {
                            name: result.name.as_ref().or(elem.name.as_ref()).cloned(),
                            fields,
                            node: result.node.as_ref().or(elem.node.as_ref()).cloned(),
                            rust_attributes: result
                                .rust_attributes
                                .as_ref()
                                .or(elem.rust_attributes.as_ref())
                                .cloned(),
                        }))
                    }
                    (Type::Array(lhs), Type::Array(rhs)) => Type::Array(if *lhs == Type::Void {
                        rhs
                    } else if *rhs == Type::Void {
                        lhs
                    } else {
                        Self::common_target_type_for_type_list(
                            [(*lhs).clone(), (*rhs).clone()].into_iter(),
                        )
                        .into()
                    }),
                    (Type::Color, Type::Brush) | (Type::Brush, Type::Color) => Type::Brush,
                    (Type::Float32, Type::Int32) | (Type::Int32, Type::Float32) => Type::Float32,
                    (target_type, expr_ty) => {
                        if expr_ty.can_convert(&target_type) {
                            target_type
                        } else if target_type.can_convert(&expr_ty)
                            || (expr_ty.default_unit().is_some()
                                && matches!(target_type, Type::Float32 | Type::Int32))
                        {
                            // in the or case: The `0` literal.
                            expr_ty
                        } else {
                            // otherwise, use the target type and let further conversion report an error
                            target_type
                        }
                    }
                }
            }
        })
    }
}

/// Perform the lookup
fn lookup_qualified_name_node(
    node: syntax_nodes::QualifiedName,
    ctx: &mut LookupCtx,
    phase: LookupPhase,
) -> Option<LookupResult> {
    let mut it = node
        .children_with_tokens()
        .filter(|n| n.kind() == SyntaxKind::Identifier)
        .filter_map(|n| n.into_token());

    let first = if let Some(first) = it.next() {
        first
    } else {
        // There must be at least one member (parser should ensure that)
        debug_assert!(ctx.diag.has_errors());
        return None;
    };

    ctx.current_token = Some(first.clone().into());
    let first_str = crate::parser::normalize_identifier(first.text());
    let global_lookup = crate::lookup::global_lookup();
    let result = match global_lookup.lookup(ctx, &first_str) {
        None => {
            if let Some(minus_pos) = first.text().find('-') {
                // Attempt to recover if the user wanted to write "-" for minus
                let first_str = &first.text()[0..minus_pos];
                if global_lookup
                    .lookup(ctx, &crate::parser::normalize_identifier(first_str))
                    .is_some()
                {
                    ctx.diag.push_error(format!("Unknown unqualified identifier '{}'. Use space before the '-' if you meant a subtraction", first.text()), &node);
                    return None;
                }
            }
            for (prefix, e) in
                [("self", ctx.component_scope.last()), ("root", ctx.component_scope.first())]
            {
                if let Some(e) = e {
                    if e.lookup(ctx, &first_str).is_some() {
                        ctx.diag.push_error(format!("Unknown unqualified identifier '{0}'. Did you mean '{prefix}.{0}'?", first.text()), &node);
                        return None;
                    }
                }
            }

            if it.next().is_some() {
                ctx.diag.push_error(format!("Cannot access id '{}'", first.text()), &node);
            } else {
                ctx.diag.push_error(
                    format!("Unknown unqualified identifier '{}'", first.text()),
                    &node,
                );
            }
            return None;
        }
        Some(x) => x,
    };

    if let Some(depr) = result.deprecated() {
        ctx.diag.push_property_deprecation_warning(&first_str, depr, &first);
    }

    match result {
        LookupResult::Expression { expression: Expression::ElementReference(e), .. } => {
            continue_lookup_within_element(&e.upgrade().unwrap(), &mut it, node, ctx)
        }
        LookupResult::Expression {
            expression: Expression::RepeaterModelReference { .. }, ..
        } if matches!(phase, LookupPhase::ResolvingTwoWayBindings) => {
            ctx.diag.push_error(
                "Two-way bindings to model data is not supported yet".to_string(),
                &node,
            );
            None
        }
        result => maybe_lookup_object(result, it, ctx),
    }
}

fn continue_lookup_within_element(
    elem: &ElementRc,
    it: &mut impl Iterator<Item = crate::parser::SyntaxToken>,
    node: syntax_nodes::QualifiedName,
    ctx: &mut LookupCtx,
) -> Option<LookupResult> {
    let second = if let Some(second) = it.next() {
        second
    } else if matches!(ctx.property_type, Type::ElementReference) {
        return Some(Expression::ElementReference(Rc::downgrade(elem)).into());
    } else {
        // Try to recover in case we wanted to access a property
        let mut rest = String::new();
        if let Some(LookupResult::Expression {
            expression: Expression::PropertyReference(nr),
            ..
        }) = crate::lookup::InScopeLookup.lookup(ctx, &elem.borrow().id)
        {
            let e = nr.element();
            let e_borrowed = e.borrow();
            let mut id = e_borrowed.id.as_str();
            if id.is_empty() {
                if ctx.component_scope.last().is_some_and(|x| Rc::ptr_eq(&e, x)) {
                    id = "self";
                } else if ctx.component_scope.first().is_some_and(|x| Rc::ptr_eq(&e, x)) {
                    id = "root";
                } else if ctx.component_scope.iter().nth_back(1).is_some_and(|x| Rc::ptr_eq(&e, x))
                {
                    id = "parent";
                }
            };
            if !id.is_empty() {
                rest =
                    format!(". Use '{id}.{}' to access the property with the same name", nr.name());
            }
        } else if let Some(LookupResult::Expression {
            expression: Expression::EnumerationValue(value),
            ..
        }) = crate::lookup::ReturnTypeSpecificLookup.lookup(ctx, &elem.borrow().id)
        {
            rest = format!(
                ". Use '{}.{value}' to access the enumeration value",
                value.enumeration.name
            );
        }
        ctx.diag.push_error(format!("Cannot take reference of an element{rest}"), &node);
        return None;
    };
    let prop_name = crate::parser::normalize_identifier(second.text());

    let lookup_result = elem.borrow().lookup_property(&prop_name);
    let local_to_component = lookup_result.is_local_to_component && ctx.is_local_element(elem);

    if lookup_result.property_type.is_property_type() {
        if !local_to_component && lookup_result.property_visibility == PropertyVisibility::Private {
            ctx.diag.push_error(format!("The property '{}' is private. Annotate it with 'in', 'out' or 'in-out' to make it accessible from other components", second.text()), &second);
            return None;
        } else if lookup_result.property_visibility == PropertyVisibility::Fake {
            ctx.diag.push_error(
                "This special property can only be used to make a binding and cannot be accessed"
                    .to_string(),
                &second,
            );
            return None;
        } else if lookup_result.resolved_name != prop_name.as_str() {
            ctx.diag.push_property_deprecation_warning(
                &prop_name,
                &lookup_result.resolved_name,
                &second,
            );
        } else if let Some(deprecated) =
            crate::lookup::check_deprecated_stylemetrics(elem, ctx, &prop_name)
        {
            ctx.diag.push_property_deprecation_warning(&prop_name, &deprecated, &second);
        }
        let prop = Expression::PropertyReference(NamedReference::new(
            elem,
            lookup_result.resolved_name.to_smolstr(),
        ));
        maybe_lookup_object(prop.into(), it, ctx)
    } else if matches!(lookup_result.property_type, Type::Callback { .. }) {
        if let Some(x) = it.next() {
            ctx.diag.push_error("Cannot access fields of callback".into(), &x)
        }
        Some(LookupResult::Callable(LookupResultCallable::Callable(Callable::Callback(
            NamedReference::new(elem, lookup_result.resolved_name.to_smolstr()),
        ))))
    } else if let Type::Function(fun) = lookup_result.property_type {
        if lookup_result.property_visibility == PropertyVisibility::Private && !local_to_component {
            let message = format!("The function '{}' is private. Annotate it with 'public' to make it accessible from other components", second.text());
            if !lookup_result.is_local_to_component {
                ctx.diag.push_error(message, &second);
            } else {
                ctx.diag.push_warning(message+". Note: this used to be allowed in previous version, but this should be considered an error", &second);
            }
        } else if lookup_result.property_visibility == PropertyVisibility::Protected
            && !local_to_component
            && !(lookup_result.is_in_direct_base
                && ctx.component_scope.first().is_some_and(|x| Rc::ptr_eq(x, elem)))
        {
            ctx.diag.push_error(format!("The function '{}' is protected", second.text()), &second);
        }
        if let Some(x) = it.next() {
            ctx.diag.push_error("Cannot access fields of a function".into(), &x)
        }
        let callable = match lookup_result.builtin_function {
            Some(builtin) => Callable::Builtin(builtin),
            None => Callable::Function(NamedReference::new(
                elem,
                lookup_result.resolved_name.to_smolstr(),
            )),
        };
        if matches!(fun.args.first(), Some(Type::ElementReference)) {
            LookupResult::Callable(LookupResultCallable::MemberFunction {
                base: Expression::ElementReference(Rc::downgrade(elem)),
                base_node: Some(NodeOrToken::Node(node.into())),
                member: Box::new(LookupResultCallable::Callable(callable)),
            })
            .into()
        } else {
            LookupResult::from(callable).into()
        }
    } else {
        let mut err = |extra: &str| {
            let what = match &elem.borrow().base_type {
                ElementType::Global => {
                    let global = elem.borrow().enclosing_component.upgrade().unwrap();
                    assert!(global.is_global());
                    format!("'{}'", global.id)
                }
                ElementType::Component(c) => format!("Element '{}'", c.id),
                ElementType::Builtin(b) => format!("Element '{}'", b.name),
                ElementType::Native(_) => unreachable!("the native pass comes later"),
                ElementType::Error => {
                    assert!(ctx.diag.has_errors());
                    return;
                }
            };
            ctx.diag.push_error(
                format!("{} does not have a property '{}'{}", what, second.text(), extra),
                &second,
            );
        };
        if let Some(minus_pos) = second.text().find('-') {
            // Attempt to recover if the user wanted to write "-"
            if elem
                .borrow()
                .lookup_property(&crate::parser::normalize_identifier(&second.text()[0..minus_pos]))
                .property_type
                != Type::Invalid
            {
                err(". Use space before the '-' if you meant a subtraction");
                return None;
            }
        }
        err("");
        None
    }
}

fn maybe_lookup_object(
    mut base: LookupResult,
    it: impl Iterator<Item = crate::parser::SyntaxToken>,
    ctx: &mut LookupCtx,
) -> Option<LookupResult> {
    for next in it {
        let next_str = crate::parser::normalize_identifier(next.text());
        ctx.current_token = Some(next.clone().into());
        match base.lookup(ctx, &next_str) {
            Some(r) => {
                base = r;
            }
            None => {
                if let Some(minus_pos) = next.text().find('-') {
                    if base.lookup(ctx, &SmolStr::new(&next.text()[0..minus_pos])).is_some() {
                        ctx.diag.push_error(format!("Cannot access the field '{}'. Use space before the '-' if you meant a subtraction", next.text()), &next);
                        return None;
                    }
                }

                match base {
                    LookupResult::Callable(LookupResultCallable::Callable(Callable::Callback(
                        ..,
                    ))) => ctx.diag.push_error("Cannot access fields of callback".into(), &next),
                    LookupResult::Callable(..) => {
                        ctx.diag.push_error("Cannot access fields of a function".into(), &next)
                    }
                    LookupResult::Enumeration(enumeration) => ctx.diag.push_error(
                        format!(
                            "'{}' is not a member of the enum {}",
                            next.text(),
                            enumeration.name
                        ),
                        &next,
                    ),

                    LookupResult::Namespace(ns) => {
                        ctx.diag.push_error(
                            format!("'{}' is not a member of the namespace {}", next.text(), ns),
                            &next,
                        );
                    }
                    LookupResult::Expression { expression, .. } => {
                        let ty_descr = match expression.ty() {
                            Type::Struct { .. } => String::new(),
                            Type::Float32
                                if ctx.property_type == Type::Model
                                    && matches!(
                                        expression,
                                        Expression::NumberLiteral(_, Unit::None),
                                    ) =>
                            {
                                // usually something like `0..foo`
                                format!(" of float. Range expressions are not supported in Slint, but you can use an integer as a model to repeat something multiple time. Eg: `for i in {}`", next.text())
                            }

                            ty => format!(" of {ty}"),
                        };
                        ctx.diag.push_error(
                            format!("Cannot access the field '{}'{}", next.text(), ty_descr),
                            &next,
                        );
                    }
                }
                return None;
            }
        }
    }
    Some(base)
}

/// Go through all the two way binding and resolve them first
fn resolve_two_way_bindings(
    doc: &Document,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    for component in doc.inner_components.iter() {
        recurse_elem_with_scope(
            &component.root_element,
            ComponentScope(vec![]),
            &mut |elem, scope| {
                for (prop_name, binding) in &elem.borrow().bindings {
                    let mut binding = binding.borrow_mut();
                    if let Expression::Uncompiled(node) =
                        binding.expression.ignore_debug_hooks().clone()
                    {
                        if let Some(n) = syntax_nodes::TwoWayBinding::new(node.clone()) {
                            let lhs_lookup = elem.borrow().lookup_property(prop_name);
                            if !lhs_lookup.is_valid() {
                                // An attempt to resolve this already failed when trying to resolve the property type
                                assert!(diag.has_errors());
                                continue;
                            }
                            let mut lookup_ctx = LookupCtx {
                                property_name: Some(prop_name.as_str()),
                                property_type: lhs_lookup.property_type.clone(),
                                component_scope: &scope.0,
                                diag,
                                arguments: vec![],
                                type_register,
                                type_loader: None,
                                current_token: Some(node.clone().into()),
                                local_variables: vec![],
                            };

                            binding.expression = Expression::Invalid;

                            if let Some(nr) = resolve_two_way_binding(n, &mut lookup_ctx) {
                                binding.two_way_bindings.push(nr.clone());

                                nr.element()
                                    .borrow()
                                    .property_analysis
                                    .borrow_mut()
                                    .entry(nr.name().clone())
                                    .or_default()
                                    .is_linked = true;

                                if matches!(
                                    lhs_lookup.property_visibility,
                                    PropertyVisibility::Private | PropertyVisibility::Output
                                ) && !lhs_lookup.is_local_to_component
                                {
                                    // invalid property assignment should have been reported earlier
                                    assert!(diag.has_errors() || elem.borrow().is_legacy_syntax);
                                    continue;
                                }

                                // Check the compatibility.
                                let mut rhs_lookup =
                                    nr.element().borrow().lookup_property(nr.name());
                                if rhs_lookup.property_type == Type::Invalid {
                                    // An attempt to resolve this already failed when trying to resolve the property type
                                    assert!(diag.has_errors());
                                    continue;
                                }
                                rhs_lookup.is_local_to_component &=
                                    lookup_ctx.is_local_element(&nr.element());

                                if !rhs_lookup.is_valid_for_assignment() {
                                    match (
                                        lhs_lookup.property_visibility,
                                        rhs_lookup.property_visibility,
                                    ) {
                                        (PropertyVisibility::Input, PropertyVisibility::Input)
                                            if !lhs_lookup.is_local_to_component =>
                                        {
                                            assert!(rhs_lookup.is_local_to_component);
                                            marked_linked_read_only(elem, prop_name);
                                        }
                                        (
                                            PropertyVisibility::Output
                                            | PropertyVisibility::Private,
                                            PropertyVisibility::Output | PropertyVisibility::Input,
                                        ) => {
                                            assert!(lhs_lookup.is_local_to_component);
                                            marked_linked_read_only(elem, prop_name);
                                        }
                                        (PropertyVisibility::Input, PropertyVisibility::Output)
                                            if !lhs_lookup.is_local_to_component =>
                                        {
                                            assert!(!rhs_lookup.is_local_to_component);
                                            marked_linked_read_only(elem, prop_name);
                                        }
                                        _ => {
                                            if lookup_ctx.is_legacy_component() {
                                                diag.push_warning(
                                                    format!(
                                                        "Link to a {} property is deprecated",
                                                        rhs_lookup.property_visibility
                                                    ),
                                                    &node,
                                                );
                                            } else {
                                                diag.push_error(
                                                    format!(
                                                        "Cannot link to a {} property",
                                                        rhs_lookup.property_visibility
                                                    ),
                                                    &node,
                                                )
                                            }
                                        }
                                    }
                                } else if !lhs_lookup.is_valid_for_assignment() {
                                    if rhs_lookup.is_local_to_component
                                        && rhs_lookup.property_visibility
                                            == PropertyVisibility::InOut
                                    {
                                        if lookup_ctx.is_legacy_component() {
                                            debug_assert!(!diag.is_empty()); // warning should already be reported
                                        } else {
                                            diag.push_error(
                                                "Cannot link input property".into(),
                                                &node,
                                            );
                                        }
                                    } else if rhs_lookup.property_visibility
                                        == PropertyVisibility::InOut
                                    {
                                        diag.push_warning("Linking input properties to input output properties is deprecated".into(), &node);
                                        marked_linked_read_only(&nr.element(), nr.name());
                                    } else {
                                        // This is allowed, but then the rhs must also become read only.
                                        marked_linked_read_only(&nr.element(), nr.name());
                                    }
                                }
                            }
                        }
                    }
                }
            },
        );
    }

    fn marked_linked_read_only(elem: &ElementRc, prop_name: &str) {
        elem.borrow()
            .property_analysis
            .borrow_mut()
            .entry(prop_name.into())
            .or_default()
            .is_linked_to_read_only = true;
    }
}

pub fn resolve_two_way_binding(
    node: syntax_nodes::TwoWayBinding,
    ctx: &mut LookupCtx,
) -> Option<NamedReference> {
    let Some(n) = node.Expression().QualifiedName() else {
        ctx.diag.push_error(
            "The expression in a two way binding must be a property reference".into(),
            &node.Expression(),
        );
        return None;
    };

    let Some(r) = lookup_qualified_name_node(n, ctx, LookupPhase::ResolvingTwoWayBindings) else {
        assert!(ctx.diag.has_errors());
        return None;
    };

    // If type is invalid, error has already been reported,  when inferring, the error will be reported by the inferring code
    let report_error = !matches!(
        ctx.property_type,
        Type::InferredProperty | Type::InferredCallback | Type::Invalid
    );
    match r {
        LookupResult::Expression { expression: Expression::PropertyReference(n), .. } => {
            if report_error && n.ty() != ctx.property_type {
                ctx.diag.push_error(
                    "The property does not have the same type as the bound property".into(),
                    &node,
                );
            }
            Some(n)
        }
        LookupResult::Callable(LookupResultCallable::Callable(Callable::Callback(n))) => {
            if report_error && n.ty() != ctx.property_type {
                ctx.diag.push_error("Cannot bind to a callback".into(), &node);
                None
            } else {
                Some(n)
            }
        }
        LookupResult::Callable(..) => {
            if report_error {
                ctx.diag.push_error("Cannot bind to a function".into(), &node);
            }
            None
        }
        _ => {
            ctx.diag.push_error(
                "The expression in a two way binding must be a property reference".into(),
                &node,
            );
            None
        }
    }
}

/// For connection to callback aliases, some check are to be performed later
fn check_callback_alias_validity(
    node: &syntax_nodes::CallbackConnection,
    elem: &ElementRc,
    name: &str,
    diag: &mut BuildDiagnostics,
) {
    let elem_borrow = elem.borrow();
    let Some(decl) = elem_borrow.property_declarations.get(name) else {
        if let ElementType::Component(c) = &elem_borrow.base_type {
            check_callback_alias_validity(node, &c.root_element, name, diag);
        }
        return;
    };
    let Some(b) = elem_borrow.bindings.get(name) else { return };
    // `try_borrow` because we might be called for the current binding
    let Some(alias) = b.try_borrow().ok().and_then(|b| b.two_way_bindings.first().cloned()) else {
        return;
    };

    if alias.element().borrow().base_type == ElementType::Global {
        diag.push_error(
            "Can't assign a local callback handler to an alias to a global callback".into(),
            &node.child_token(SyntaxKind::Identifier).unwrap(),
        );
    }
    if let Type::Callback(callback) = &decl.property_type {
        let num_arg = node.DeclaredIdentifier().count();
        if num_arg > callback.args.len() {
            diag.push_error(
                format!(
                    "'{name}' only has {} arguments, but {num_arg} were provided",
                    callback.args.len(),
                ),
                &node.child_token(SyntaxKind::Identifier).unwrap(),
            );
        }
    }
}
