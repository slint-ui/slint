// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! Passes that resolve the property binding expression.
//!
//! Before this pass, all the expression are of type Expression::Uncompiled,
//! and there should no longer be Uncompiled expression after this pass.
//!
//! Most of the code for the resolving actually lies in the expression_tree module

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::*;
use crate::langtype::{ElementType, Type};
use crate::lookup::{LookupCtx, LookupObject, LookupResult};
use crate::object_tree::*;
use crate::parser::{identifier_text, syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode};
use crate::typeregister::TypeRegister;
use core::num::IntErrorKind;
use std::collections::HashMap;
use std::rc::Rc;

/// This represents a scope for the Component, where Component is the repeated component, but
/// does not represent a component in the .slint file
#[derive(Clone)]
struct ComponentScope(Vec<ElementRc>);

fn resolve_expression(
    expr: &mut Expression,
    property_name: Option<&str>,
    property_type: Type,
    scope: &ComponentScope,
    type_register: &TypeRegister,
    type_loader: &crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    if let Expression::Uncompiled(node) = expr {
        let mut lookup_ctx = LookupCtx {
            property_name,
            property_type,
            component_scope: &scope.0,
            diag,
            arguments: vec![],
            type_register,
            type_loader: Some(type_loader),
            current_token: None,
        };

        let new_expr = match node.kind() {
            SyntaxKind::CallbackConnection => {
                Expression::from_callback_connection(node.clone().into(), &mut lookup_ctx)
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
                assert!(diag.has_error(), "Two way binding should have been resolved already  (property: {property_name:?})");
                Expression::Invalid
            }
            _ => {
                debug_assert!(diag.has_error());
                Expression::Invalid
            }
        };
        *expr = new_expr;
    }
}

pub fn resolve_expressions(
    doc: &Document,
    type_loader: &crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    resolve_two_way_bindings(doc, &doc.local_registry, diag);

    for component in doc.inner_components.iter() {
        let scope = ComponentScope(vec![]);

        recurse_elem(&component.root_element, &scope, &mut |elem, scope| {
            let mut new_scope = scope.clone();
            let mut is_repeated = elem.borrow().repeated.is_some();
            new_scope.0.push(elem.clone());
            visit_element_expressions(elem, |expr, property_name, property_type| {
                if is_repeated {
                    // The first expression is always the model and it needs to be resolved with the parent scope
                    debug_assert!(matches!(
                        elem.borrow().repeated.as_ref().unwrap().model,
                        Expression::Invalid
                    )); // should be Invalid because it is taken by the visit_element_expressions function
                    resolve_expression(
                        expr,
                        property_name,
                        property_type(),
                        scope,
                        &doc.local_registry,
                        type_loader,
                        diag,
                    );
                    is_repeated = false;
                } else {
                    resolve_expression(
                        expr,
                        property_name,
                        property_type(),
                        &new_scope,
                        &doc.local_registry,
                        type_loader,
                        diag,
                    )
                }
            });
            new_scope
        })
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
            .child_node(SyntaxKind::Expression)
            .map(|n| Self::from_expression_node(n.into(), ctx))
            .or_else(|| {
                node.child_node(SyntaxKind::CodeBlock)
                    .map(|c| Self::from_codeblock_node(c.into(), ctx))
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
            assert!(ctx.diag.has_error());
            e
        }
    }

    fn from_codeblock_node(node: syntax_nodes::CodeBlock, ctx: &mut LookupCtx) -> Expression {
        debug_assert_eq!(node.kind(), SyntaxKind::CodeBlock);

        let mut statements_or_exprs = node
            .children()
            .filter_map(|n| match n.kind() {
                SyntaxKind::Expression => Some(Self::from_expression_node(n.into(), ctx)),
                SyntaxKind::ReturnStatement => Some(Self::from_return_statement(n.into(), ctx)),
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

        Expression::CodeBlock(statements_or_exprs)
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
        Self::from_codeblock_node(node.CodeBlock(), ctx).maybe_convert_to(
            ctx.return_type().clone(),
            &node,
            ctx.diag,
        )
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

    fn from_expression_node(node: syntax_nodes::Expression, ctx: &mut LookupCtx) -> Self {
        node.Expression()
            .map(|n| Self::from_expression_node(n, ctx))
            .or_else(|| node.AtImageUrl().map(|n| Self::from_at_image_url_node(n, ctx)))
            .or_else(|| node.AtGradient().map(|n| Self::from_at_gradient(n, ctx)))
            .or_else(|| node.AtTr().map(|n| Self::from_at_tr(n, ctx)))
            .or_else(|| {
                node.QualifiedName().map(|n| {
                    let exp =
                        Self::from_qualified_name_node(n.clone(), ctx, LookupPhase::default());
                    if matches!(exp.ty(), Type::Function { .. } | Type::Callback { .. }) {
                        ctx.diag.push_error(
                            format!(
                                "'{}' must be called. Did you forgot the '()'?",
                                QualifiedTypeName::from_node(n.clone())
                            ),
                            &n,
                        )
                    }
                    exp
                })
            })
            .or_else(|| {
                node.child_text(SyntaxKind::StringLiteral).map(|s| {
                    crate::literals::unescape_string(&s).map(Self::StringLiteral).unwrap_or_else(
                        || {
                            ctx.diag.push_error("Cannot parse string literal".into(), &node);
                            Self::Invalid
                        },
                    )
                })
            })
            .or_else(|| {
                node.child_text(SyntaxKind::NumberLiteral)
                    .map(crate::literals::parse_number_literal)
                    .transpose()
                    .unwrap_or_else(|e| {
                        ctx.diag.push_error(e, &node);
                        Some(Self::Invalid)
                    })
            })
            .or_else(|| {
                node.child_text(SyntaxKind::ColorLiteral).map(|s| {
                    crate::literals::parse_color_literal(&s)
                        .map(|i| Expression::Cast {
                            from: Box::new(Expression::NumberLiteral(i as _, Unit::None)),
                            to: Type::Color,
                        })
                        .unwrap_or_else(|| {
                            ctx.diag.push_error("Invalid color literal".into(), &node);
                            Self::Invalid
                        })
                })
            })
            .or_else(|| {
                node.FunctionCallExpression().map(|n| Self::from_function_call_node(n, ctx))
            })
            .or_else(|| node.MemberAccess().map(|n| Self::from_member_access_node(n, ctx)))
            .or_else(|| node.IndexExpression().map(|n| Self::from_index_expression_node(n, ctx)))
            .or_else(|| node.SelfAssignment().map(|n| Self::from_self_assignment_node(n, ctx)))
            .or_else(|| node.BinaryExpression().map(|n| Self::from_binary_expression_node(n, ctx)))
            .or_else(|| {
                node.UnaryOpExpression().map(|n| Self::from_unaryop_expression_node(n, ctx))
            })
            .or_else(|| {
                node.ConditionalExpression().map(|n| Self::from_conditional_expression_node(n, ctx))
            })
            .or_else(|| node.ObjectLiteral().map(|n| Self::from_object_literal_node(n, ctx)))
            .or_else(|| node.Array().map(|n| Self::from_array_node(n, ctx)))
            .or_else(|| node.CodeBlock().map(|n| Self::from_codeblock_node(n, ctx)))
            .or_else(|| node.StringTemplate().map(|n| Self::from_string_template_node(n, ctx)))
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
                    .map(|i| i.0.to_string_lossy().to_string())
                    .unwrap_or_else(|| {
                        crate::pathutils::join(
                            &crate::pathutils::dirname(node.source_file.path()),
                            path,
                        )
                        .map(|p| p.to_string_lossy().to_string())
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
                assert!(ctx.diag.has_error());
                None
            }
        };

        Expression::ImageReference {
            resource_ref: ImageReference::AbsolutePath(absolute_source_path),
            source_location: Some(node.to_source_location()),
            nine_slice,
        }
    }

    fn from_at_gradient(node: syntax_nodes::AtGradient, ctx: &mut LookupCtx) -> Self {
        enum GradKind {
            Linear { angle: Box<Expression> },
            Radial,
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
            if subs.next().map_or(false, |s| s.kind() != SyntaxKind::Comma) {
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
            if comma.as_ref().map_or(false, |s| s.kind() != SyntaxKind::Comma) {
                ctx.diag.push_error(
                    "'circle' must be followed by a comma".into(),
                    comma.as_ref().map_or(&node, |x| x as &dyn Spanned),
                );
                return Expression::Invalid;
            }
            GradKind::Radial
        } else {
            // Parser should have ensured we have one of the linear or radial gradient
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
                        stops.push((col, e.maybe_convert_to(Type::Float32, &n, ctx.diag)))
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
            while let Some(mut p) = string[pos..].find(|x| x == '{' || x == '}') {
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

        let plural = plural.unwrap_or((String::new(), Expression::NumberLiteral(1., Unit::None)));

        let get_component_name = || {
            ctx.component_scope
                .first()
                .and_then(|e| e.borrow().enclosing_component.upgrade())
                .map(|c| c.id.clone())
        };

        Expression::FunctionCall {
            function: Box::new(Expression::BuiltinFunctionReference(
                BuiltinFunction::Translate,
                Some(node.to_source_location()),
            )),
            arguments: vec![
                Expression::StringLiteral(string),
                Expression::StringLiteral(context.or_else(get_component_name).unwrap_or_default()),
                Expression::StringLiteral(domain),
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
        let mut it = node
            .children_with_tokens()
            .filter(|n| n.kind() == SyntaxKind::Identifier)
            .filter_map(|n| n.into_token());

        let first = if let Some(first) = it.next() {
            first
        } else {
            // There must be at least one member (parser should ensure that)
            debug_assert!(ctx.diag.has_error());
            return Self::Invalid;
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
                        return Expression::Invalid;
                    }
                }
                for (prefix, e) in
                    [("self", ctx.component_scope.last()), ("root", ctx.component_scope.first())]
                {
                    if let Some(e) = e {
                        if e.lookup(ctx, &first_str).is_some() {
                            ctx.diag.push_error(format!("Unknown unqualified identifier '{0}'. Did you mean '{prefix}.{0}'?", first.text()), &node);
                            return Expression::Invalid;
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
                return Expression::Invalid;
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
                expression: r @ Expression::CallbackReference(..), ..
            } => {
                if let Some(x) = it.next() {
                    ctx.diag.push_error("Cannot access fields of callback".into(), &x)
                }
                r
            }
            LookupResult::Expression {
                expression: r @ Expression::FunctionReference(..), ..
            } => {
                if let Some(x) = it.next() {
                    ctx.diag.push_error("Cannot access fields of a function".into(), &x)
                }
                r
            }
            LookupResult::Enumeration(enumeration) => {
                if let Some(next_identifier) = it.next() {
                    match enumeration
                        .lookup(ctx, &crate::parser::normalize_identifier(next_identifier.text()))
                    {
                        Some(LookupResult::Expression { expression, .. }) => {
                            maybe_lookup_object(expression, it, ctx)
                        }
                        _ => {
                            ctx.diag.push_error(
                                format!(
                                    "'{}' is not a member of the enum {}",
                                    next_identifier.text(),
                                    enumeration.name
                                ),
                                &next_identifier,
                            );
                            Expression::Invalid
                        }
                    }
                } else {
                    ctx.diag.push_error("Cannot take reference to an enum".to_string(), &node);
                    Expression::Invalid
                }
            }
            LookupResult::Expression {
                expression: Expression::RepeaterModelReference { .. },
                ..
            } if matches!(phase, LookupPhase::ResolvingTwoWayBindings) => {
                ctx.diag.push_error(
                    "Two-way bindings to model data is not supported yet".to_string(),
                    &node,
                );
                Expression::Invalid
            }
            LookupResult::Expression { expression, .. } => maybe_lookup_object(expression, it, ctx),
            LookupResult::Namespace(_) => {
                if let Some(next_identifier) = it.next() {
                    match result
                        .lookup(ctx, &crate::parser::normalize_identifier(next_identifier.text()))
                    {
                        Some(LookupResult::Expression { expression, .. }) => {
                            maybe_lookup_object(expression, it, ctx)
                        }
                        _ => {
                            ctx.diag.push_error(
                                format!(
                                    "'{}' is not a member of the namespace {}",
                                    next_identifier.text(),
                                    first_str
                                ),
                                &next_identifier,
                            );
                            Expression::Invalid
                        }
                    }
                } else {
                    ctx.diag.push_error("Cannot take reference to a namespace".to_string(), &node);
                    Expression::Invalid
                }
            }
        }
    }

    fn from_function_call_node(
        node: syntax_nodes::FunctionCallExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let mut arguments = Vec::new();

        let mut sub_expr = node.Expression();

        let function = sub_expr.next().map_or(Self::Invalid, |n| {
            // Treat the QualifiedName separately so we can catch the uses of uncalled signal
            n.QualifiedName()
                .map(|qn| Self::from_qualified_name_node(qn, ctx, LookupPhase::default()))
                .unwrap_or_else(|| Self::from_expression_node(n, ctx))
        });

        let sub_expr = sub_expr.map(|n| {
            (Self::from_expression_node(n.clone(), ctx), Some(NodeOrToken::from((*n).clone())))
        });

        let mut adjust_arg_count = 0;

        let function = match function {
            Expression::BuiltinMacroReference(mac, n) => {
                arguments.extend(sub_expr);
                return crate::builtin_macros::lower_macro(mac, n, arguments.into_iter(), ctx.diag);
            }
            Expression::MemberFunction { base, base_node, member } => {
                arguments.push((*base, base_node));
                adjust_arg_count = 1;
                member
            }
            _ => Box::new(function),
        };
        arguments.extend(sub_expr);

        let arguments = match function.ty() {
            Type::Function { args, .. } | Type::Callback { args, .. } => {
                if arguments.len() != args.len() {
                    ctx.diag.push_error(
                        format!(
                            "The callback or function expects {} arguments, but {} are provided",
                            args.len() - adjust_arg_count,
                            arguments.len() - adjust_arg_count,
                        ),
                        &node,
                    );
                    arguments.into_iter().map(|x| x.0).collect()
                } else {
                    arguments
                        .into_iter()
                        .zip(args.iter())
                        .map(|((e, node), ty)| e.maybe_convert_to(ty.clone(), &node, ctx.diag))
                        .collect()
                }
            }
            Type::Invalid => {
                debug_assert!(ctx.diag.has_error());
                arguments.into_iter().map(|x| x.0).collect()
            }
            _ => {
                ctx.diag.push_error("The expression is not a function".into(), &node);
                arguments.into_iter().map(|x| x.0).collect()
            }
        };

        Expression::FunctionCall {
            function,
            arguments,
            source_location: Some(node.to_source_location()),
        }
    }

    fn from_member_access_node(
        node: syntax_nodes::MemberAccess,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let base = Self::from_expression_node(node.Expression(), ctx);
        maybe_lookup_object(base, node.child_token(SyntaxKind::Identifier).into_iter(), ctx)
    }

    fn from_self_assignment_node(
        node: syntax_nodes::SelfAssignment,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let (lhs_n, rhs_n) = node.Expression();
        let mut lhs = Self::from_expression_node(lhs_n.clone(), ctx);
        let op = None
            .or_else(|| node.child_token(SyntaxKind::PlusEqual).and(Some('+')))
            .or_else(|| node.child_token(SyntaxKind::MinusEqual).and(Some('-')))
            .or_else(|| node.child_token(SyntaxKind::StarEqual).and(Some('*')))
            .or_else(|| node.child_token(SyntaxKind::DivEqual).and(Some('/')))
            .or_else(|| node.child_token(SyntaxKind::Equal).and(Some('=')))
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
                        format!("the {}= operation cannot be done on a {}", op, ty),
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
        let op = None
            .or_else(|| node.child_token(SyntaxKind::Plus).and(Some('+')))
            .or_else(|| node.child_token(SyntaxKind::Minus).and(Some('-')))
            .or_else(|| node.child_token(SyntaxKind::Star).and(Some('*')))
            .or_else(|| node.child_token(SyntaxKind::Div).and(Some('/')))
            .or_else(|| node.child_token(SyntaxKind::LessEqual).and(Some('≤')))
            .or_else(|| node.child_token(SyntaxKind::GreaterEqual).and(Some('≥')))
            .or_else(|| node.child_token(SyntaxKind::LAngle).and(Some('<')))
            .or_else(|| node.child_token(SyntaxKind::RAngle).and(Some('>')))
            .or_else(|| node.child_token(SyntaxKind::EqualEqual).and(Some('=')))
            .or_else(|| node.child_token(SyntaxKind::NotEqual).and(Some('!')))
            .or_else(|| node.child_token(SyntaxKind::AndAnd).and(Some('&')))
            .or_else(|| node.child_token(SyntaxKind::OrOr).and(Some('|')))
            .unwrap_or('_');

        let (lhs_n, rhs_n) = node.Expression();
        let lhs = Self::from_expression_node(lhs_n.clone(), ctx);
        let rhs = Self::from_expression_node(rhs_n.clone(), ctx);

        let expected_ty = match operator_class(op) {
            OperatorClass::ComparisonOp => {
                Self::common_target_type_for_type_list([lhs.ty(), rhs.ty()].iter().cloned())
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

        let op = None
            .or_else(|| node.child_token(SyntaxKind::Plus).and(Some('+')))
            .or_else(|| node.child_token(SyntaxKind::Minus).and(Some('-')))
            .or_else(|| node.child_token(SyntaxKind::Bang).and(Some('!')))
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
                assert!(ctx.diag.has_error());
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
        if !matches!(ty, Type::Array(_) | Type::Invalid) {
            ctx.diag.push_error(format!("{} is not an indexable type", ty), &node);
        }
        Expression::ArrayIndex { array: Box::new(array_expr), index: Box::new(index_expr) }
    }

    fn from_object_literal_node(
        node: syntax_nodes::ObjectLiteral,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let values: HashMap<String, Expression> = node
            .ObjectMember()
            .map(|n| {
                (
                    identifier_text(&n).unwrap_or_default(),
                    Expression::from_expression_node(n.Expression(), ctx),
                )
            })
            .collect();
        let ty = Type::Struct {
            fields: values.iter().map(|(k, v)| (k.clone(), v.ty())).collect(),
            name: None,
            node: None,
            rust_attributes: None,
        };
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
                    (
                        Type::Struct {
                            fields: mut result_fields,
                            name: result_name,
                            node: result_node,
                            rust_attributes,
                        },
                        Type::Struct {
                            fields: elem_fields,
                            name: elem_name,
                            node: elem_node,
                            rust_attributes: derived,
                        },
                    ) => {
                        for (elem_name, elem_ty) in elem_fields.into_iter() {
                            match result_fields.entry(elem_name) {
                                std::collections::btree_map::Entry::Vacant(free_entry) => {
                                    free_entry.insert(elem_ty);
                                }
                                std::collections::btree_map::Entry::Occupied(
                                    mut existing_field,
                                ) => {
                                    *existing_field.get_mut() =
                                        Self::common_target_type_for_type_list(
                                            [existing_field.get().clone(), elem_ty].into_iter(),
                                        );
                                }
                            }
                        }
                        Type::Struct {
                            name: result_name.or(elem_name),
                            fields: result_fields,
                            node: result_node.or(elem_node),
                            rust_attributes: rust_attributes.or(derived),
                        }
                    }
                    (Type::Array(lhs), Type::Array(rhs)) => Type::Array(if *lhs == Type::Void {
                        rhs
                    } else if *rhs == Type::Void {
                        lhs
                    } else {
                        Self::common_target_type_for_type_list([*lhs, *rhs].into_iter()).into()
                    }),
                    (Type::Color, Type::Brush) | (Type::Brush, Type::Color) => Type::Brush,
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

fn continue_lookup_within_element(
    elem: &ElementRc,
    it: &mut impl Iterator<Item = crate::parser::SyntaxToken>,
    node: syntax_nodes::QualifiedName,
    ctx: &mut LookupCtx,
) -> Expression {
    let second = if let Some(second) = it.next() {
        second
    } else if matches!(ctx.property_type, Type::ElementReference) {
        return Expression::ElementReference(Rc::downgrade(elem));
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
                if ctx.component_scope.last().map_or(false, |x| Rc::ptr_eq(&e, x)) {
                    id = "self";
                } else if ctx.component_scope.first().map_or(false, |x| Rc::ptr_eq(&e, x)) {
                    id = "root";
                } else if ctx
                    .component_scope
                    .iter()
                    .nth_back(1)
                    .map_or(false, |x| Rc::ptr_eq(&e, x))
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
        return Expression::Invalid;
    };
    let prop_name = crate::parser::normalize_identifier(second.text());

    let lookup_result = elem.borrow().lookup_property(&prop_name);
    let local_to_component = lookup_result.is_local_to_component && ctx.is_local_element(elem);

    if lookup_result.property_type.is_property_type() {
        if !local_to_component && lookup_result.property_visibility == PropertyVisibility::Private {
            ctx.diag.push_error(format!("The property '{}' is private. Annotate it with 'in', 'out' or 'in-out' to make it accessible from other components", second.text()), &second);
            return Expression::Invalid;
        }
        if lookup_result.resolved_name != prop_name {
            ctx.diag.push_property_deprecation_warning(
                &prop_name,
                &lookup_result.resolved_name,
                &second,
            );
        }
        let prop =
            Expression::PropertyReference(NamedReference::new(elem, &lookup_result.resolved_name));
        maybe_lookup_object(prop, it, ctx)
    } else if matches!(lookup_result.property_type, Type::Callback { .. }) {
        if let Some(x) = it.next() {
            ctx.diag.push_error("Cannot access fields of callback".into(), &x)
        }
        Expression::CallbackReference(
            NamedReference::new(elem, &lookup_result.resolved_name),
            Some(NodeOrToken::Token(second)),
        )
    } else if matches!(lookup_result.property_type, Type::Function { .. }) {
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
                && ctx.component_scope.first().map_or(false, |x| Rc::ptr_eq(x, elem)))
        {
            ctx.diag.push_error(format!("The function '{}' is protected", second.text()), &second);
        }
        if let Some(x) = it.next() {
            ctx.diag.push_error("Cannot access fields of a function".into(), &x)
        }
        if let Some(f) =
            elem.borrow().base_type.lookup_member_function(&lookup_result.resolved_name)
        {
            // builtin member function
            Expression::MemberFunction {
                base: Box::new(Expression::ElementReference(Rc::downgrade(elem))),
                base_node: Some(NodeOrToken::Node(node.into())),
                member: Expression::BuiltinFunctionReference(f, Some(second.to_source_location()))
                    .into(),
            }
        } else {
            Expression::FunctionReference(
                NamedReference::new(elem, &lookup_result.resolved_name),
                Some(NodeOrToken::Token(second)),
            )
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
                    assert!(ctx.diag.has_error());
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
                return Expression::Invalid;
            }
        }
        err("");
        Expression::Invalid
    }
}

fn maybe_lookup_object(
    mut base: Expression,
    it: impl Iterator<Item = crate::parser::SyntaxToken>,
    ctx: &mut LookupCtx,
) -> Expression {
    for next in it {
        let next_str = crate::parser::normalize_identifier(next.text());
        ctx.current_token = Some(next.clone().into());
        match base.lookup(ctx, &next_str) {
            Some(LookupResult::Expression { expression, .. }) => {
                base = expression;
            }
            _ => {
                if let Some(minus_pos) = next.text().find('-') {
                    if base.lookup(ctx, &next.text()[0..minus_pos]).is_some() {
                        ctx.diag.push_error(format!("Cannot access the field '{}'. Use space before the '-' if you meant a subtraction", next.text()), &next);
                        return Expression::Invalid;
                    }
                }
                let ty_descr = match base.ty() {
                    Type::Struct { .. } => String::new(),
                    ty => format!(" of {}", ty),
                };
                ctx.diag.push_error(
                    format!("Cannot access the field '{}'{}", next.text(), ty_descr),
                    &next,
                );
                return Expression::Invalid;
            }
        }
    }
    base
}

/// Go through all the two way binding and resolve them first
fn resolve_two_way_bindings(
    doc: &Document,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    for component in doc.inner_components.iter() {
        let scope = ComponentScope(vec![]);

        recurse_elem(&component.root_element, &scope, &mut |elem, scope| {
            let mut new_scope = scope.clone();
            new_scope.0.push(elem.clone());
            for (prop_name, binding) in &elem.borrow().bindings {
                let mut binding = binding.borrow_mut();
                if let Expression::Uncompiled(node) = binding.expression.clone() {
                    if let Some(n) = syntax_nodes::TwoWayBinding::new(node.clone()) {
                        let lhs_lookup = elem.borrow().lookup_property(prop_name);
                        if lhs_lookup.property_type == Type::Invalid {
                            // An attempt to resolve this already failed when trying to resolve the property type
                            assert!(diag.has_error());
                            continue;
                        }
                        let mut lookup_ctx = LookupCtx {
                            property_name: Some(prop_name.as_str()),
                            property_type: lhs_lookup.property_type.clone(),
                            component_scope: &new_scope.0,
                            diag,
                            arguments: vec![],
                            type_register,
                            type_loader: None,
                            current_token: Some(node.clone().into()),
                        };

                        binding.expression = Expression::Invalid;

                        if let Some(nr) = resolve_two_way_binding(n, &mut lookup_ctx) {
                            binding.two_way_bindings.push(nr.clone());

                            nr.element()
                                .borrow()
                                .property_analysis
                                .borrow_mut()
                                .entry(nr.name().to_string())
                                .or_default()
                                .is_linked = true;

                            // Check the compatibility.
                            let mut rhs_lookup = nr.element().borrow().lookup_property(nr.name());
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
                                        PropertyVisibility::Output | PropertyVisibility::Private,
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
                                    && rhs_lookup.property_visibility == PropertyVisibility::InOut
                                {
                                    if lookup_ctx.is_legacy_component() {
                                        debug_assert!(!diag.is_empty()); // warning should already be reported
                                    } else {
                                        diag.push_error("Cannot link input property".into(), &node);
                                    }
                                } else {
                                    // This is allowed, but then the rhs must also become read only.
                                    marked_linked_read_only(&nr.element(), nr.name());
                                }
                            }
                        }
                    }
                }
            }
            new_scope
        })
    }

    fn marked_linked_read_only(elem: &ElementRc, prop_name: &str) {
        elem.borrow()
            .property_analysis
            .borrow_mut()
            .entry(prop_name.to_string())
            .or_default()
            .is_linked_to_read_only = true;
    }
}

pub fn resolve_two_way_binding(
    node: syntax_nodes::TwoWayBinding,
    ctx: &mut LookupCtx,
) -> Option<NamedReference> {
    let e = if let Some(n) = node.Expression().QualifiedName() {
        Expression::from_qualified_name_node(n, ctx, LookupPhase::ResolvingTwoWayBindings)
    } else {
        ctx.diag.push_error(
            "The expression in a two way binding must be a property reference".into(),
            &node.Expression(),
        );
        return None;
    };
    // If type is invalid, error has already been reported,  when inferring, the error will be reported by the inferring code
    let report_error = !matches!(
        ctx.property_type,
        Type::InferredProperty | Type::InferredCallback | Type::Invalid
    );
    let ty = e.ty();
    match e {
        Expression::PropertyReference(n) => {
            if report_error && ty != ctx.property_type {
                ctx.diag.push_error(
                    "The property does not have the same type as the bound property".into(),
                    &node,
                );
            }
            Some(n)
        }
        Expression::CallbackReference(n, _) => {
            if report_error && ty != ctx.property_type {
                ctx.diag.push_error("Cannot bind to a callback".into(), &node);
                None
            } else {
                Some(n)
            }
        }
        Expression::FunctionReference(..) => {
            if report_error {
                ctx.diag.push_error("Cannot bind to a function".into(), &node);
            }
            None
        }
        Expression::Invalid => {
            debug_assert!(ctx.diag.has_error());
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
