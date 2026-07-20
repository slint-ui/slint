// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore depr descr idents shiftbehavior unaryop Unshiftable uppercased
//! This pass resolves the property binding expressions.
//!
//! Before this pass, all the expression are of type Expression::Uncompiled,
//! and there should no longer be Uncompiled expression after this pass.
//!
//! Most of the code for the resolving actually lies in the expression_tree module

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::*;
use crate::langtype;
use crate::langtype::{ElementType, KeyboardModifiers, Struct, StructName, Type};
use crate::lookup::{LookupCtx, LookupObject, LookupResult, LookupResultCallable};
use crate::object_tree::*;
use crate::parser::{NodeOrToken, SyntaxKind, SyntaxNode, identifier_text, syntax_nodes};
use crate::symbol_counters::SymbolCounters;
use crate::typeregister::TypeRegister;
use core::num::IntErrorKind;
use i_slint_common::for_each_physical_keys;
use smol_str::{SmolStr, ToSmolStr};
use std::collections::BTreeMap;
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;

mod remove_noop;

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
            expected_type: Type::default(),
            component_scope: scope,
            diag,
            symbol_counters: type_loader.symbol_counters.clone(),
            arguments: Vec::new(),
            type_register,
            type_loader: Some(type_loader),
            current_token: None,
            local_variables: Vec::new(),
        };
        lookup_ctx.expected_type = lookup_ctx.return_type().clone();

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
                    .maybe_convert_to(
                        lookup_ctx.property_type.clone(),
                        node,
                        lookup_ctx.diag,
                        &lookup_ctx.symbol_counters,
                    )
            }
            SyntaxKind::BindingExpression => {
                Expression::from_binding_expression_node(node.clone(), &mut lookup_ctx)
            }
            SyntaxKind::PropertyChangedCallback => {
                let node = syntax_nodes::PropertyChangedCallback::from(node.clone());
                if let Some(code_block_node) = node.CodeBlock() {
                    Expression::from_codeblock_node(code_block_node, &mut lookup_ctx)
                } else if let Some(expr_node) = node.Expression() {
                    Expression::from_expression_node(expr_node, &mut lookup_ctx)
                } else {
                    assert!(diag.has_errors());
                    Expression::Invalid
                }
            }
            SyntaxKind::TwoWayBinding => {
                assert!(
                    diag.has_errors(),
                    "Two way binding should have been resolved already  (property: {property_name:?})"
                );
                Expression::Invalid
            }
            SyntaxKind::AtKeys => {
                Expression::from_at_keys_node(node.clone().into(), &mut lookup_ctx)
            }
            SyntaxKind::AtPhysicalKeys => {
                Expression::from_at_physical_keys_node(node.clone().into(), &mut lookup_ctx)
            }
            _ => {
                debug_assert!(diag.has_errors());
                Expression::Invalid
            }
        };
        match expr {
            Expression::DebugHook { expression, .. } => **expression = new_expr,
            _ => *expr = new_expr,
        }
    // Specifically used to resolve match expressions
    } else if let Expression::BinaryExpression { lhs, rhs, op } = expr {
        let op = *op;
        let rhs_node =
            if let Expression::Uncompiled(node) = rhs.as_ref() { Some(node.clone()) } else { None };

        resolve_expression(
            elem,
            lhs,
            property_name,
            Type::Invalid,
            scope,
            type_register,
            type_loader,
            diag,
        );
        resolve_expression(
            elem,
            rhs,
            property_name,
            lhs.ty(),
            scope,
            type_register,
            type_loader,
            diag,
        );
        if op == '=' {
            let is_literal = matches!(
                rhs.as_ref(),
                Expression::NumberLiteral(..)
                    | Expression::StringLiteral(..)
                    | Expression::BoolLiteral(..)
                    | Expression::EnumerationValue(..)
            );
            let is_cast = matches!(rhs.as_ref(), Expression::Cast { .. });
            let is_valid_cast = matches!(
                rhs.as_ref(),
                Expression::Cast { from, to, .. }
                    if matches!(from.as_ref(), Expression::NumberLiteral(..))
                        && matches!(to, Type::Color | Type::Int32)
            );
            if let Expression::NumberLiteral(val, unit) = rhs.as_ref()
                && *unit == Unit::None
                && val.fract() != 0.0
                && let Some(node) = &rhs_node
            {
                diag.push_warning("Floating point comparison is not recommended".into(), node);
            }

            if let Some(node) = rhs_node {
                if is_literal || is_valid_cast {
                    // pass
                } else if is_cast {
                    diag.push_error("Cannot perform type conversion".into(), &node);
                } else {
                    diag.push_error("Match expressions must be literal values".into(), &node);
                }
            }
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
    for component in doc.inner_components.iter() {
        recurse_elem_with_scope(
            &component.root_element,
            ComponentScope(Vec::new()),
            &mut |elem, scope| {
                // Resolve the model expression (of a `for`) with the parent
                // scope, and before the two-way bindings below so they can
                // type-check field accesses against the model row type.
                if elem.borrow().repeated.is_some() {
                    debug_assert!(scope.0.len() > 1);
                    let parent_scope = &scope.0[..scope.0.len() - 1];
                    visit_repeater_model_expression(elem, |expr, property_name, property_type| {
                        resolve_expression(
                            elem,
                            expr,
                            property_name,
                            property_type(),
                            parent_scope,
                            &doc.local_registry,
                            type_loader,
                            diag,
                        );
                    });
                }

                resolve_two_way_bindings_for_element(elem, &scope.0, &doc.local_registry, diag);

                visit_element_expressions_excluding_repeater_model(
                    elem,
                    |expr, property_name, property_type| {
                        resolve_expression(
                            elem,
                            expr,
                            property_name,
                            property_type(),
                            &scope.0,
                            &doc.local_registry,
                            type_loader,
                            diag,
                        );
                    },
                );
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
            e.maybe_convert_to(ctx.property_type.clone(), &node, ctx.diag, &ctx.symbol_counters)
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
                SyntaxKind::Expression => {
                    Some((n.clone(), Self::from_expression_node(n.into(), ctx)))
                }
                SyntaxKind::ReturnStatement => {
                    Some((n.clone(), Self::from_return_statement(n.into(), ctx)))
                }
                SyntaxKind::LetStatement => {
                    Some((n.clone(), Self::from_let_statement(n.into(), ctx)))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        remove_noop::remove_from_codeblock(&mut statements_or_exprs, ctx.diag);

        let mut statements_or_exprs = statements_or_exprs
            .into_iter()
            .map(|(_node, statement_or_expr)| statement_or_expr)
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
            expr = expr.maybe_convert_to(
                common_return_type.clone(),
                &node,
                ctx.diag,
                &ctx.symbol_counters,
            );
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

        let declared_ty = node.Type().map(|ty| type_from_node(ty, ctx.diag, ctx.type_register));
        let value = match &declared_ty {
            Some(t) => ctx.with_expected_type(t.clone(), |ctx| {
                Self::from_expression_node(node.Expression(), ctx)
            }),
            None => Self::from_expression_node(node.Expression(), ctx),
        };
        let ty = declared_ty.unwrap_or_else(|| value.ty());

        // we can get the last scope exists, because each codeblock creates a new scope and we are inside a codeblock here by necessity
        ctx.local_variables.last_mut().unwrap().push((name.clone(), ty.clone()));

        let value =
            Box::new(value.maybe_convert_to(ty.clone(), &node, ctx.diag, &ctx.symbol_counters));

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
            let e = ctx
                .with_expected_type(return_type.clone(), |ctx| Self::from_expression_node(n, ctx));
            Box::new(e.maybe_convert_to(return_type, &node, ctx.diag, &ctx.symbol_counters))
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
                &ctx.symbol_counters,
            )
        } else if let Some(expr_node) = node.Expression() {
            Self::from_expression_node(expr_node, ctx).maybe_convert_to(
                ctx.return_type().clone(),
                &node,
                ctx.diag,
                &ctx.symbol_counters,
            )
        } else {
            Expression::Invalid
        }
    }

    fn from_function(node: syntax_nodes::Function, ctx: &mut LookupCtx) -> Expression {
        ctx.arguments = node
            .ArgumentDeclaration()
            .map(|x| identifier_text(&x.DeclaredIdentifier()).unwrap_or_default())
            .collect();
        let Some(code_block) = node.CodeBlock() else {
            debug_assert!(ctx.diag.has_errors());
            return Expression::Invalid;
        };
        Self::from_codeblock_node(code_block, ctx).maybe_convert_to(
            ctx.return_type().clone(),
            &node,
            ctx.diag,
            &ctx.symbol_counters,
        )
    }

    pub fn from_expression_node(node: syntax_nodes::Expression, ctx: &mut LookupCtx) -> Self {
        // This function recurses for nested expressions. Dispatch with early returns
        // instead of a `find_map` closure: in unoptimized builds, every arm of a match
        // producing a value gets its own stack slot for the resulting `Expression`,
        // adding up to a frame so large that deeply nested expressions overflow the
        // stack. A `return` writes directly into the return slot instead.
        for child in node.children_with_tokens() {
            match child {
                NodeOrToken::Node(node) => match node.kind() {
                    SyntaxKind::Expression => return Self::from_expression_node(node.into(), ctx),
                    SyntaxKind::AtImageUrl => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("@image-url() expressions are", &node);
                        return Self::from_at_image_url_node(node.into(), ctx);
                    }
                    SyntaxKind::AtGradient => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("@gradient expressions are", &node);
                        return Self::from_at_gradient(node.into(), ctx);
                    }
                    SyntaxKind::AtTr => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("@tr() expressions are", &node);
                        return Self::from_at_tr(node.into(), ctx);
                    }
                    SyntaxKind::AtMarkdown => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("@markdown() expressions are", &node);
                        return Self::from_at_markdown(node.into(), ctx);
                    }
                    SyntaxKind::AtKeys => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("@keys() expressions are", &node);
                        return Self::from_at_keys_node(node.into(), ctx);
                    }
                    SyntaxKind::AtPhysicalKeys => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("@physical-keys() expressions are", &node);
                        return Self::from_at_physical_keys_node(node.into(), ctx);
                    }
                    SyntaxKind::QualifiedName => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Identifier references are", &node);
                        return Self::from_qualified_name_node(node.clone().into(), ctx);
                    }
                    SyntaxKind::FunctionCallExpression => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Function calls are", &node);
                        return Self::from_function_call_node(node.into(), ctx);
                    }
                    SyntaxKind::MemberAccess => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Member access expressions are", &node);
                        return Self::from_member_access_node(node.into(), ctx);
                    }
                    SyntaxKind::IndexExpression => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Index expressions are", &node);
                        return Self::from_index_expression_node(node.into(), ctx);
                    }
                    SyntaxKind::SelfAssignment => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Self-assignment expressions are", &node);
                        return Self::from_self_assignment_node(node.into(), ctx);
                    }
                    SyntaxKind::BinaryExpression => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Binary expressions are", &node);
                        return Self::from_binary_expression_node(node.into(), ctx);
                    }
                    SyntaxKind::UnaryOpExpression => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Unary expressions are", &node);
                        return Self::from_unaryop_expression_node(node.into(), ctx);
                    }
                    SyntaxKind::ConditionalExpression => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Conditional expressions are", &node);
                        return Self::from_conditional_expression_node(node.into(), ctx);
                    }
                    SyntaxKind::ObjectLiteral => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Object literal expressions are", &node);
                        return Self::from_object_literal_node(node.into(), ctx);
                    }
                    SyntaxKind::Array => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Array expressions are", &node);
                        return Self::from_array_node(node.into(), ctx);
                    }
                    SyntaxKind::CodeBlock => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Code blocks are", &node);
                        return Self::from_codeblock_node(node.into(), ctx);
                    }
                    SyntaxKind::StringTemplate => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("String interpolation expressions are", &node);
                        return Self::from_string_template_node(node.into(), ctx);
                    }
                    _ => {}
                },
                NodeOrToken::Token(token) => match token.kind() {
                    SyntaxKind::StringLiteral => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("String literals are", &token);
                        return crate::literals::unescape_string_reporting(
                            Some(&token),
                            ctx.diag,
                            &token,
                        )
                        .map(Self::StringLiteral)
                        .unwrap_or(Self::Invalid);
                    }
                    SyntaxKind::NumberLiteral => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Number literals are", &token);
                        return crate::literals::parse_number_literal(token.text().into())
                            .map(|(value, unit)| {
                                let (value, unit) = unit.normalize(value);
                                Expression::NumberLiteral(value, unit)
                            })
                            .unwrap_or_else(|e| {
                                ctx.diag.push_error(e.to_string(), &node);
                                Self::Invalid
                            });
                    }
                    SyntaxKind::ColorLiteral => {
                        #[cfg(feature = "slint-sc")]
                        ctx.diag.slint_sc_error("Color literals are", &token);
                        return i_slint_common::color_parsing::parse_color_literal(token.text())
                            .map(|i| Expression::Cast {
                                from: Box::new(Expression::NumberLiteral(i as _, Unit::None)),
                                to: Type::Color,
                            })
                            .unwrap_or_else(|| {
                                ctx.diag.push_error("Invalid color literal".into(), &node);
                                Self::Invalid
                            });
                    }

                    _ => {}
                },
            }
        }
        Self::Invalid
    }

    fn from_at_image_url_node(node: syntax_nodes::AtImageUrl, ctx: &mut LookupCtx) -> Self {
        let Some(s) = crate::literals::unescape_string_reporting(
            node.child_token(SyntaxKind::StringLiteral).as_ref(),
            ctx.diag,
            &node,
        ) else {
            return Self::Invalid;
        };

        if s.is_empty() {
            return Expression::ImageReference {
                resource_ref: ImageReference::None,
                source_location: Some(node.to_source_location()),
                nine_slice: None,
            };
        }

        let resource_ref = if s.starts_with("data:") {
            ImageReference::DataUri(s)
        } else {
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
            ImageReference::from_resolved(absolute_source_path)
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
            resource_ref,
            source_location: Some(node.to_source_location()),
            nine_slice,
        }
    }

    pub fn from_at_gradient(node: syntax_nodes::AtGradient, ctx: &mut LookupCtx) -> Self {
        enum GradKind {
            Linear {
                angle: Box<Expression>,
            },
            Radial {
                center: Option<(Box<Expression>, Box<Expression>)>,
                radius: Option<Box<Expression>>,
            },
            Conic {
                from_angle: Box<Expression>,
                center: Option<(Box<Expression>, Box<Expression>)>,
            },
        }

        let all_subs: Vec<_> = node
            .children_with_tokens()
            .filter(|n| matches!(n.kind(), SyntaxKind::Comma | SyntaxKind::Expression))
            .collect();

        let grad_token = node.child_token(SyntaxKind::Identifier).unwrap();
        let grad_text = grad_token.text();

        // Helper: parse two consecutive length expressions at positions idx and idx+1
        let parse_at_center = |idx: usize,
                               ctx: &mut LookupCtx|
         -> Option<(Box<Expression>, Box<Expression>)> {
            let cx_node = all_subs.get(idx)?;
            let cy_node = all_subs.get(idx + 1)?;
            if cx_node.kind() != SyntaxKind::Expression || cy_node.kind() != SyntaxKind::Expression
            {
                return None;
            }
            let cx_syn = syntax_nodes::Expression::from(cx_node.as_node().unwrap().clone());
            let cy_syn = syntax_nodes::Expression::from(cy_node.as_node().unwrap().clone());
            let cx =
                Box::new(Expression::from_expression_node(cx_syn.clone(), ctx).maybe_convert_to(
                    Type::LogicalLength,
                    &cx_syn,
                    ctx.diag,
                    &ctx.symbol_counters,
                ));
            let cy =
                Box::new(Expression::from_expression_node(cy_syn.clone(), ctx).maybe_convert_to(
                    Type::LogicalLength,
                    &cy_syn,
                    ctx.diag,
                    &ctx.symbol_counters,
                ));
            Some((cx, cy))
        };

        let (grad_kind, stops_start_idx) = if grad_text.starts_with("linear") {
            let angle_expr = match all_subs.first() {
                Some(e) if e.kind() == SyntaxKind::Expression => {
                    syntax_nodes::Expression::from(e.as_node().unwrap().clone())
                }
                _ => {
                    ctx.diag.push_error("Expected angle expression".into(), &node);
                    return Expression::Invalid;
                }
            };
            if all_subs.get(1).is_none_or(|s| s.kind() != SyntaxKind::Comma) {
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
                    &ctx.symbol_counters,
                ),
            );
            (GradKind::Linear { angle }, 2)
        } else if grad_text.starts_with("radial") {
            if !all_subs.first().is_some_and(|n| {
                matches!(n, NodeOrToken::Node(node) if node.text().to_string().trim() == "circle")
            }) {
                ctx.diag.push_error("Expected 'circle': currently, only @radial-gradient(circle, ...) are supported".into(), &node);
                return Expression::Invalid;
            }
            // CSS syntax: `circle [<radius>] [at <x> <y>]` — radius before center, no keyword.
            let mut idx = 1;

            // Parse optional radius (a length expression that is not the "at" keyword).
            // Only consume the node when it actually resolves to a length-compatible type;
            // a colour keyword like `blue` must not silently become a failed conversion.
            let radius = if all_subs.get(idx).is_some_and(|n| {
                n.kind() == SyntaxKind::Expression
                    && !matches!(n, NodeOrToken::Node(node) if node.text().to_string().trim() == "at")
            }) {
                let r = all_subs.get(idx).unwrap();
                let r_syn = syntax_nodes::Expression::from(r.as_node().unwrap().clone());
                let expr = Expression::from_expression_node(r_syn.clone(), ctx);
                if matches!(expr.ty(), Type::LogicalLength | Type::Float32 | Type::Int32) {
                    let radius = Box::new(
                        expr.maybe_convert_to(Type::LogicalLength, &r_syn, ctx.diag, &ctx.symbol_counters),
                    );
                    idx += 1;
                    Some(radius)
                } else {
                    None
                }
            } else {
                None
            };

            // Parse optional "at <x> <y>".
            let center = if all_subs.get(idx).is_some_and(
                |n| matches!(n, NodeOrToken::Node(node) if node.text().to_string().trim() == "at"),
            ) {
                let center = parse_at_center(idx + 1, ctx);
                if center.is_none() {
                    ctx.diag.push_error(
                        "Expected two length values after 'at'".into(),
                        all_subs.get(idx).unwrap(),
                    );
                    return Expression::Invalid;
                }
                idx += 3; // consumed "at x y"
                center
            } else {
                None
            };

            let stops_start = if all_subs.get(idx).is_none() {
                idx
            } else if all_subs.get(idx).is_some_and(|s| s.kind() == SyntaxKind::Comma) {
                idx + 1
            } else {
                if idx == 1 {
                    let message = "'circle' must be followed by a comma, a radius, or 'at'".into();
                    if let Some(error_node) = all_subs.get(idx) {
                        ctx.diag.push_error(message, error_node);
                    } else {
                        ctx.diag.push_error(message, &node);
                    }
                } else {
                    ctx.diag
                        .push_error("gradient header must be followed by a comma".into(), &node);
                }
                return Expression::Invalid;
            };
            (GradKind::Radial { center, radius }, stops_start)
        } else if grad_text.starts_with("conic") {
            // Parse optional "from <angle>" and/or "at <x> <y>" before the comma
            let mut idx = 0usize;
            let from_angle = if all_subs.first().is_some_and(|n| {
                matches!(n, NodeOrToken::Node(node) if node.text().to_string().trim() == "from")
            }) {
                // Parse "from <angle>" syntax
                let angle_expr = match all_subs.get(1) {
                    Some(e) if e.kind() == SyntaxKind::Expression => {
                        syntax_nodes::Expression::from(e.as_node().unwrap().clone())
                    }
                    _ => {
                        ctx.diag.push_error("Expected angle expression after 'from'".into(), &node);
                        return Expression::Invalid;
                    }
                };
                let angle = Box::new(
                    Expression::from_expression_node(angle_expr.clone(), ctx).maybe_convert_to(
                        Type::Angle,
                        &angle_expr,
                        ctx.diag, &ctx.symbol_counters),
                );
                idx = 2; // consumed "from" and angle
                angle
            } else {
                // Default to 0deg when "from" is omitted
                Box::new(Expression::NumberLiteral(0., Unit::Deg))
            };

            // Parse optional "at <x> <y>" after the optional "from <angle>"
            let center = if all_subs.get(idx).is_some_and(
                |n| matches!(n, NodeOrToken::Node(node) if node.text().to_string().trim() == "at"),
            ) {
                let center = parse_at_center(idx + 1, ctx);
                if center.is_none() {
                    ctx.diag.push_error(
                        "Expected two length values after 'at'".into(),
                        all_subs.get(idx).unwrap(),
                    );
                    return Expression::Invalid;
                }
                idx += 3; // consumed "at", x, y
                center
            } else {
                None
            };

            // Expect a comma after the header (if any header elements were present)
            if (idx > 0) && all_subs.get(idx).is_none_or(|s| s.kind() != SyntaxKind::Comma) {
                ctx.diag.push_error("gradient header must be followed by a comma".into(), &node);
                return Expression::Invalid;
            }
            let stops_start = if idx > 0 { idx + 1 } else { 0 };
            (GradKind::Conic { from_angle, center }, stops_start)
        } else {
            // Parser should have ensured we have one of the linear, radial or conic gradient
            panic!("Not a gradient {grad_text:?}");
        };

        let mut stops = Vec::new();
        enum Stop {
            Empty,
            Color(Expression),
            Finished,
        }
        let mut current_stop = Stop::Empty;
        for n in all_subs.iter().skip(stops_start_idx) {
            if n.kind() == SyntaxKind::Comma {
                match std::mem::replace(&mut current_stop, Stop::Empty) {
                    Stop::Empty => {
                        ctx.diag.push_error("Expected expression".into(), n);
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
                // To facilitate color literal conversion, adjust the expected type.
                let e = ctx.with_expected_type(Type::Color, |ctx| {
                    Expression::from_expression_node(n.as_node().unwrap().clone().into(), ctx)
                });
                match std::mem::replace(&mut current_stop, Stop::Finished) {
                    Stop::Empty => {
                        current_stop = Stop::Color(e.maybe_convert_to(
                            Type::Color,
                            n,
                            ctx.diag,
                            &ctx.symbol_counters,
                        ))
                    }
                    Stop::Finished => {
                        ctx.diag.push_error("Expected comma".into(), n);
                        break;
                    }
                    Stop::Color(col) => {
                        let stop_type = match &grad_kind {
                            GradKind::Conic { .. } => Type::Angle,
                            _ => Type::Float32,
                        };
                        stops.push((
                            col,
                            e.maybe_convert_to(stop_type, n, ctx.diag, &ctx.symbol_counters),
                        ))
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
            GradKind::Radial { center, radius } => {
                Expression::RadialGradient { center, radius, stops }
            }
            GradKind::Conic { from_angle, center } => {
                // Normalize stop angles to 0-1 range by dividing by 360deg
                let normalized_stops = stops
                    .into_iter()
                    .map(|(color, angle_expr)| {
                        let angle_typed = angle_expr.maybe_convert_to(
                            Type::Angle,
                            &node,
                            ctx.diag,
                            &ctx.symbol_counters,
                        );
                        let normalized_pos = Expression::BinaryExpression {
                            lhs: Box::new(angle_typed),
                            rhs: Box::new(Expression::NumberLiteral(360., Unit::Deg)),
                            op: '/',
                        };
                        (color, normalized_pos)
                    })
                    .collect();

                // Convert from_angle to degrees (don't normalize to 0-1)
                let from_angle_degrees =
                    from_angle.maybe_convert_to(Type::Angle, &node, ctx.diag, &ctx.symbol_counters);

                Expression::ConicGradient {
                    from_angle: Box::new(from_angle_degrees),
                    center,
                    stops: normalized_stops,
                }
            }
        }
    }

    fn from_at_markdown(node: syntax_nodes::AtMarkdown, ctx: &mut LookupCtx) -> Expression {
        let mut raw_exprs: Vec<(Expression, crate::parser::SyntaxNode)> = Vec::new();
        let mut source_map = crate::literals::StringLiteralSourceMap::new();
        use i_slint_common::styled_text::MARKDOWN_INTERPOLATION_PLACEHOLDER as PLACEHOLDER;

        let push_and_check =
            |token: &crate::parser::SyntaxToken,
             source_map: &mut crate::literals::StringLiteralSourceMap,
             diag: &mut crate::diagnostics::BuildDiagnostics| {
                let before = source_map.as_str().len();
                source_map.push(token, diag);
                for (offset, _) in source_map.as_str()[before..].match_indices(PLACEHOLDER) {
                    source_map.report(
                        diag,
                        "\\u{e541} is reserved for @markdown interpolation".into(),
                        (before + offset)..(before + offset + PLACEHOLDER.len_utf8()),
                        &node,
                    );
                }
            };

        for n in node.children_with_tokens() {
            if n.kind() == SyntaxKind::StringLiteral {
                push_and_check(n.as_token().unwrap(), &mut source_map, ctx.diag);
            } else if n.kind() == SyntaxKind::StringTemplate {
                for n in n.as_node().unwrap().children_with_tokens() {
                    if n.kind() == SyntaxKind::StringLiteral {
                        push_and_check(n.as_token().unwrap(), &mut source_map, ctx.diag);
                    } else if n.kind() == SyntaxKind::Expression {
                        let expr_node = n.into_node().unwrap();
                        let expr = Expression::from_expression_node(expr_node.clone().into(), ctx);
                        source_map.push_raw_char(PLACEHOLDER, expr_node.to_source_location());
                        raw_exprs.push((expr, expr_node));
                    }
                }
            }
        }

        let markdown = source_map.as_str();
        let placeholder_positions: Vec<usize> =
            markdown.match_indices(PLACEHOLDER).map(|(pos, _)| pos).collect();

        // Replace each placeholder with an ASCII string of the same byte length
        // and re-parse.
        // pulldown_cmark treats `<zzz>` as inline HTML (unlike the private-use char),
        // so errors reveal interpolations inside HTML tag structure.
        const PROBE: &str = "zzz";
        const _: () = assert!(PROBE.len() == PLACEHOLDER.len_utf8());
        let probe = markdown.replace(PLACEHOLDER, PROBE);

        let (_, parse_errors) = i_slint_common::styled_text::parse_interpolated::<
            &[i_slint_common::styled_text::StyledTextParagraph],
        >(&probe, &[]);

        let mut color_indices = std::collections::BTreeSet::new();

        for e in &parse_errors {
            let placeholders_in_range = |r: &core::ops::Range<usize>| -> Vec<usize> {
                placeholder_positions
                    .iter()
                    .enumerate()
                    .filter(|(_, pos)| **pos >= r.start && **pos < r.end)
                    .map(|(idx, _)| idx)
                    .collect()
            };

            if let Some(r) = e.range() {
                let hits = placeholders_in_range(&r);

                // InvalidColor("zzz") at a placeholder position →
                // this interpolation is a color attribute value.
                if i_slint_common::styled_text::invalid_color_value(e) == Some(PROBE)
                    && !hits.is_empty()
                {
                    color_indices.extend(hits);
                    continue;
                }

                // Other errors overlapping a placeholder mean interpolation
                // inside HTML tag structure.
                if !hits.is_empty() {
                    source_map.report(
                        ctx.diag,
                        "Interpolation (`\\{}`) is not allowed inside HTML tags".into(),
                        r,
                        &node,
                    );
                } else {
                    source_map.report(ctx.diag, e.to_string(), r, &node);
                }
            } else {
                ctx.diag.push_error(e.to_string(), &node);
            }
        }

        let values = raw_exprs
            .into_iter()
            .enumerate()
            .map(|(idx, (expr, expr_node))| {
                if color_indices.contains(&idx) {
                    // Color placeholder: require Color type
                    Expression::FunctionCall {
                        function: BuiltinFunction::ColorToStyledText.into(),
                        arguments: vec![expr.maybe_convert_to(
                            Type::Color,
                            &expr_node,
                            ctx.diag,
                            &ctx.symbol_counters,
                        )],
                        source_location: Some(expr_node.to_source_location()),
                    }
                } else if expr.ty() == Type::StyledText {
                    expr
                } else {
                    Expression::FunctionCall {
                        function: BuiltinFunction::StringToStyledText.into(),
                        arguments: vec![expr.maybe_convert_to(
                            Type::String,
                            &expr_node,
                            ctx.diag,
                            &ctx.symbol_counters,
                        )],
                        source_location: Some(expr_node.to_source_location()),
                    }
                }
            })
            .collect();

        Expression::FunctionCall {
            function: BuiltinFunction::ParseMarkdown.into(),
            arguments: vec![
                Expression::StringLiteral(source_map.into_string().into()),
                Expression::Array { element_ty: Type::StyledText, values },
            ],
            source_location: Some(node.to_source_location()),
        }
    }

    fn from_at_tr(node: syntax_nodes::AtTr, ctx: &mut LookupCtx) -> Expression {
        let mut source_map = crate::literals::StringLiteralSourceMap::new();
        let Some(string_token) = node.child_token(SyntaxKind::StringLiteral) else {
            ctx.diag.push_error("Cannot parse string literal".into(), &node);
            return Expression::Invalid;
        };
        if !source_map.push(&string_token, ctx.diag) {
            return Expression::Invalid;
        }
        let string: SmolStr = source_map.as_str().into();
        let context = node.TrContext().map(|n| {
            crate::literals::unescape_string_reporting(
                n.child_token(SyntaxKind::StringLiteral).as_ref(),
                ctx.diag,
                &n,
            )
            .unwrap_or_default()
        });
        let plural = node.TrPlural().map(|pl| {
            let s = crate::literals::unescape_string_reporting(
                pl.child_token(SyntaxKind::StringLiteral).as_ref(),
                ctx.diag,
                &pl,
            )
            .unwrap_or_default();
            let n = pl.Expression();
            let expr = Expression::from_expression_node(n.clone(), ctx).maybe_convert_to(
                Type::Int32,
                &n,
                ctx.diag,
                &ctx.symbol_counters,
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
                &ctx.symbol_counters,
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
                    p += pos;
                    source_map.report(
                        ctx.diag,
                        "Unescaped trailing '{' in format string. Escape '{' with '{{'".into(),
                        p..p + 1,
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
                        source_map.report(
                            ctx.diag,
                            "Unescaped '}' in format string. Escape '}' with '}}'".into(),
                            p..p + 1,
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
                    source_map.report(
                        ctx.diag,
                        "Unterminated placeholder in format string. '{' must be escaped with '{{'"
                            .into(),
                        p..string.len(),
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
                        source_map.report(
                            ctx.diag,
                            "`{n}` placeholder can only be found in plural form".into(),
                            p..end + 1,
                            &node,
                        );
                    }
                } else {
                    source_map.report(
                        ctx.diag,
                        "Invalid '{...}' placeholder in format string. The placeholder must be a number, or braces must be escaped with '{{' and '}}'".into(),
                        p..end + 1,
                        &node,
                    );
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

        let context = context.or_else(|| {
            if !ctx.type_loader.is_some_and(|tl| {
                tl.compiler_config.default_translation_context
                    == crate::DefaultTranslationContext::None
            }) {
                // Get the component name as a default
                ctx.component_scope
                    .first()
                    .and_then(|e| e.borrow().enclosing_component.upgrade())
                    .map(|c| c.id.clone())
            } else {
                None
            }
        });

        Expression::FunctionCall {
            function: BuiltinFunction::Translate.into(),
            arguments: vec![
                Expression::StringLiteral(string),
                Expression::StringLiteral(context.unwrap_or_default()),
                Expression::StringLiteral(domain.into()),
                Expression::Array { element_ty: Type::String, values },
                plural.1,
                Expression::StringLiteral(plural.0),
            ],
            source_location: Some(node.to_source_location()),
        }
    }

    pub fn from_at_keys_node(node: syntax_nodes::AtKeys, ctx: &mut LookupCtx) -> Self {
        let mut keys = langtype::Keys::default();

        let mut key_code: Option<(SmolStr, ShiftBehavior, NodeOrToken)> = None;

        let idents_and_questions: Vec<_> = node
            .children_with_tokens()
            .filter(|n| matches!(n.kind(), SyntaxKind::Identifier | SyntaxKind::Question))
            // The first identifier is always `keys`
            .skip(1)
            .collect();

        for (index, ident_or_question) in idents_and_questions.iter().enumerate() {
            if ident_or_question.kind() == SyntaxKind::Question {
                continue;
            }
            let identifier = ident_or_question;

            let is_question = || -> bool {
                matches!(
                    idents_and_questions.get(index + 1).map(NodeOrToken::kind),
                    Some(SyntaxKind::Question)
                )
            };

            match identifier.as_token().unwrap().text() {
                "Alt" => {
                    if is_question() {
                        keys.ignore_alt = true;
                    } else {
                        keys.modifiers.alt = true;
                    }
                }
                "Control" => keys.modifiers.control = true,
                "Meta" => keys.modifiers.meta = true,
                "Shift" => {
                    if is_question() {
                        keys.ignore_shift = true;
                    } else {
                        keys.modifiers.shift = true;
                    }
                }
                key_name => {
                    if let Some((key, shiftbehavior)) = lookup_key_name(key_name) {
                        key_code = Some((
                            SmolStr::from_iter(core::iter::once(key)),
                            shiftbehavior,
                            identifier.clone(),
                        ))
                    } else {
                        // TODO: This should suggest more kinds of close matches
                        let uppercased = key_name.to_uppercase();
                        let hint = if lookup_key_name(&uppercased).is_some() {
                            // common case: @keys(Control+a) instead of @keys(Control+A)
                            format!("Use uppercase {uppercased} instead")
                        } else {
                            format!("Consider using \"{key_name}\"")
                        };
                        ctx.diag.push_error(
                            format!("{key_name} not defined in the Keys namespace\n({hint})"),
                            identifier,
                        );
                        keys.modifiers = KeyboardModifiers::default();
                        break;
                    }
                }
            }
        }

        // Handle localization issues regarding shift per-keycode
        // This only applies to keys that are in the Key namespace
        if let Some((key_code, shift_behavior, node)) = key_code {
            match shift_behavior {
                ShiftBehavior::LocalizedShiftable { shifted_hint } => {
                    if keys.ignore_shift {
                        ctx.diag.push_warning(
                            format!(
                                "{name} already implies Shift? (remove Shift?)",
                                name = node.as_token().unwrap().text()
                            ),
                            &node,
                        );
                    }
                    keys.ignore_shift = true;
                    if keys.modifiers.shift {
                        let shifted_hint = lookup_key_name(shifted_hint).map(|(shifted_code, _shift_behavior)|
                            format!("\nConsider using {shifted_hint} to match when the user types '{shifted_code}'")
                        ).unwrap_or_default();

                        ctx.diag.push_error(
                            format!(
                                "{name} implies Shift? to support different keyboard layouts\n\
                                Remove Shift to match when the user types '{key_code}'{shifted_hint}",
                                name = node.as_token().unwrap().text()
                            ),
                            &node,
                        );
                    }
                }
                // Unshiftable keys ignore the shift state in their key_code
                // No special action needed
                ShiftBehavior::Unshiftable => {}
            }
            keys.key = key_code;
        }

        // If there is a string literal, use it as the key
        if let Some(token) = node.child_token(SyntaxKind::StringLiteral)
            && let Some(key) =
                crate::literals::unescape_string_reporting(Some(&token), ctx.diag, &token)
        {
            // NFC-normalize the key string for consistent matching
            let normalizer = icu_normalizer::ComposingNormalizer::new_nfc();
            let key: SmolStr = normalizer.normalize(&key).into();

            // Validate that the string literal contains exactly one grapheme cluster
            let grapheme_count = key.graphemes(true).count();
            if grapheme_count == 0 {
                ctx.diag.push_error("Key string literal must not be empty".to_string(), &token);
            } else if grapheme_count > 1 {
                ctx.diag.push_error(
                    format!(
                        "Key string literal must contain exactly one grapheme cluster, found {grapheme_count}",
                    ),
                    &token,
                );
            }

            keys.key = key;

            let lowercase: SmolStr = keys.key.to_lowercase().into();
            if lowercase != keys.key {
                ctx.diag.push_error(
                    format!(
                        "Key string literals must currently be lowercase, use \"{lowercase}\" instead",
                    ),
                    &token,
                );
            }
        }

        Expression::Keys(keys)
    }

    pub fn from_at_physical_keys_node(
        node: syntax_nodes::AtPhysicalKeys,
        ctx: &mut LookupCtx,
    ) -> Self {
        let mut keys = langtype::Keys { is_physical: true, ..Default::default() };

        let idents_and_questions: Vec<_> = node
            .children_with_tokens()
            .filter(|n| matches!(n.kind(), SyntaxKind::Identifier | SyntaxKind::Question))
            .skip(1)
            .collect();

        for (index, ident_or_question) in idents_and_questions.iter().enumerate() {
            if ident_or_question.kind() == SyntaxKind::Question {
                continue;
            }
            let identifier = ident_or_question;

            let is_question = || -> bool {
                matches!(
                    idents_and_questions.get(index + 1).map(NodeOrToken::kind),
                    Some(SyntaxKind::Question)
                )
            };

            match identifier.as_token().unwrap().text() {
                "Alt" => {
                    if is_question() {
                        keys.ignore_alt = true;
                    } else {
                        keys.modifiers.alt = true;
                    }
                }
                "Control" => keys.modifiers.control = true,
                "Meta" => keys.modifiers.meta = true,
                "Shift" => {
                    if is_question() {
                        keys.ignore_shift = true;
                    } else {
                        keys.modifiers.shift = true;
                    }
                }
                key_name => {
                    if let Some(key) = lookup_physical_key(key_name) {
                        keys.key = key.into();
                    } else {
                        ctx.diag.push_error(
                            format!(
                                "{key_name} is not supported by @physical-keys\n\
                                Use a physical key name such as A, Digit1, BackQuote, or LeftArrow"
                            ),
                            identifier,
                        );
                        keys.modifiers = KeyboardModifiers::default();
                        break;
                    }
                }
            }
        }

        if let Some(token) = node.child_token(SyntaxKind::StringLiteral) {
            ctx.diag.push_error(
                "String literals are not supported in @physical-keys (use a key name such as A or LeftArrow)"
                    .into(),
                &token,
            );
        }

        Expression::Keys(keys)
    }

    /// Perform the lookup
    fn from_qualified_name_node(node: syntax_nodes::QualifiedName, ctx: &mut LookupCtx) -> Self {
        Self::from_lookup_result(
            lookup_qualified_name_node(node.clone(), ctx, LookupPhase::default()),
            ctx,
            &node,
        )
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
        // Convert the arguments once the parameter types are known, so a bare color/enum
        // literal in argument position resolves against its parameter type.
        let arg_nodes = sub_expr.collect::<Vec<_>>();
        let convert_args = |ctx: &mut LookupCtx, expected: &[Type]| {
            arg_nodes
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    let ty = expected.get(i).cloned().unwrap_or(Type::Invalid);
                    let e = ctx.with_expected_type(ty, |ctx| {
                        Self::from_expression_node((*n).clone(), ctx)
                    });
                    (e, Some(NodeOrToken::from((**n).clone())))
                })
                .collect::<Vec<_>>()
        };
        let Some(function) = function else {
            // Check sub expressions anyway
            convert_args(ctx, &[]);
            assert!(ctx.diag.has_errors());
            return Self::Invalid;
        };
        let LookupResult::Callable(function) = function else {
            // Check sub expressions anyway
            convert_args(ctx, &[]);
            ctx.diag.push_error("The expression is not a function".into(), &node);
            return Self::Invalid;
        };

        let mut adjust_arg_count = 0;
        let function = match function {
            LookupResultCallable::Callable(c) => c,
            LookupResultCallable::Macro(mac) => {
                arguments.extend(convert_args(ctx, &[]));
                return crate::builtin_macros::lower_macro(
                    mac,
                    &source_location,
                    arguments.into_iter(),
                    ctx.diag,
                    &ctx.symbol_counters,
                );
            }
            LookupResultCallable::MemberFunction { member, base, base_node } => {
                arguments.push((base, base_node));
                adjust_arg_count = 1;
                match *member {
                    LookupResultCallable::Callable(c) => c,
                    LookupResultCallable::Macro(mac) => {
                        arguments.extend(convert_args(ctx, &[]));
                        return crate::builtin_macros::lower_macro(
                            mac,
                            &source_location,
                            arguments.into_iter(),
                            ctx.diag,
                            &ctx.symbol_counters,
                        );
                    }
                    LookupResultCallable::MemberFunction { .. } => {
                        unreachable!()
                    }
                }
            }
        };

        match function.ty() {
            Type::Function(f) | Type::Callback(f) => {
                arguments.extend(convert_args(ctx, f.args.get(adjust_arg_count..).unwrap_or(&[])));
            }
            _ => arguments.extend(convert_args(ctx, &[])),
        }

        if matches!(&function, Callable::Callback(nr) if nr.name() == "init") {
            ctx.diag.push_warning(
                "Calling 'init' explicitly does nothing and is deprecated".into(),
                &node,
            );
        }

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
                        .map(|((e, node), ty)| {
                            e.maybe_convert_to(ty.clone(), &node, ctx.diag, &ctx.symbol_counters)
                        })
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
        let rhs = ctx.with_expected_type(expected_ty.clone(), |ctx| {
            Self::from_expression_node(rhs_n.clone(), ctx)
        });
        Expression::SelfAssignment {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs.maybe_convert_to(
                expected_ty,
                &rhs_n,
                ctx.diag,
                &ctx.symbol_counters,
            )),
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

        let op_class = operator_class(op);
        let (lhs_n, rhs_n) = node.Expression();
        // `&&`/`||` operands are bool; a comparison's rhs takes the lhs type. Setting the
        // expected type lets a bare literal resolve (or cleanly fail) at that position.
        let lhs = if op_class == OperatorClass::LogicalOp {
            ctx.with_expected_type(Type::Bool, |ctx| Self::from_expression_node(lhs_n.clone(), ctx))
        } else {
            Self::from_expression_node(lhs_n.clone(), ctx)
        };
        let rhs = match op_class {
            OperatorClass::ComparisonOp => ctx
                .with_expected_type(lhs.ty(), |ctx| Self::from_expression_node(rhs_n.clone(), ctx)),
            OperatorClass::LogicalOp => ctx.with_expected_type(Type::Bool, |ctx| {
                Self::from_expression_node(rhs_n.clone(), ctx)
            }),
            OperatorClass::ArithmeticOp => Self::from_expression_node(rhs_n.clone(), ctx),
        };

        // The conversion target for each operand; `None` keeps the operand as-is.
        // Convert both operands at a single construction site below: in unoptimized
        // builds, every `Expression::BinaryExpression { .. }` construction gets its
        // own stack slots for the operand temporaries, and this function is part of
        // the recursion over nested expressions, where large stack frames make
        // deeply nested expressions overflow the stack.
        let (lhs_target, rhs_target) = match op_class {
            OperatorClass::ComparisonOp => {
                let ty =
                    Self::common_target_type_for_type_list([lhs.ty(), rhs.ty()].iter().cloned());
                if !matches!(op, '=' | '!') && ty.as_unit_product().is_none() && ty != Type::String
                {
                    ctx.diag.push_error(format!("Values of type {ty} cannot be compared"), &node);
                }
                (Some(ty.clone()), Some(ty))
            }
            OperatorClass::LogicalOp => (Some(Type::Bool), Some(Type::Bool)),
            OperatorClass::ArithmeticOp => {
                let (lhs_ty, rhs_ty) = (lhs.ty(), rhs.ty());
                if op == '*' || op == '/' {
                    let has_unit = |ty: &Type| {
                        matches!(ty, Type::UnitProduct(_)) || ty.default_unit().is_some()
                    };
                    match (has_unit(&lhs_ty), has_unit(&rhs_ty)) {
                        (true, true) => (None, None),
                        (true, false) => (None, Some(Type::Float32)),
                        (false, true) => (Some(Type::Float32), None),
                        (false, false) => (Some(Type::Float32), Some(Type::Float32)),
                    }
                } else if op == '+' || op == '-' {
                    let expected_ty =
                        if op == '+' && (lhs_ty == Type::String || rhs_ty == Type::String) {
                            Type::String
                        } else if lhs_ty.default_unit().is_some() {
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
                    (Some(expected_ty.clone()), Some(expected_ty))
                } else {
                    unreachable!()
                }
            }
        };
        let lhs = match lhs_target {
            Some(ty) => lhs.maybe_convert_to(ty, &lhs_n, ctx.diag, &ctx.symbol_counters),
            None => lhs,
        };
        let rhs = match rhs_target {
            Some(ty) => rhs.maybe_convert_to(ty, &rhs_n, ctx.diag, &ctx.symbol_counters),
            None => rhs,
        };
        Expression::BinaryExpression { lhs: Box::new(lhs), rhs: Box::new(rhs), op }
    }

    fn from_unaryop_expression_node(
        node: syntax_nodes::UnaryOpExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let op = node
            .children_with_tokens()
            .find_map(|n| match n.kind() {
                SyntaxKind::Plus => Some('+'),
                SyntaxKind::Minus => Some('-'),
                SyntaxKind::Bang => Some('!'),
                _ => None,
            })
            .unwrap_or('_');

        let exp_n = node.Expression();
        let exp = if op == '!' {
            ctx.with_expected_type(Type::Bool, |ctx| Self::from_expression_node(exp_n, ctx))
        } else {
            Self::from_expression_node(exp_n, ctx)
        };

        let exp = match op {
            '!' => exp.maybe_convert_to(Type::Bool, &node, ctx.diag, &ctx.symbol_counters),
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
        let condition = ctx
            .with_expected_type(Type::Bool, |ctx| {
                Self::from_expression_node(condition_n.clone(), ctx)
            })
            .maybe_convert_to(Type::Bool, &condition_n, ctx.diag, &ctx.symbol_counters);
        let true_expr = Self::from_expression_node(true_expr_n.clone(), ctx);
        let false_expr = Self::from_expression_node(false_expr_n.clone(), ctx);
        let result_ty = common_expression_type(&true_expr, &false_expr);
        let true_expr = true_expr.maybe_convert_to(
            result_ty.clone(),
            &true_expr_n,
            ctx.diag,
            &ctx.symbol_counters,
        );
        let false_expr =
            false_expr.maybe_convert_to(result_ty, &false_expr_n, ctx.diag, &ctx.symbol_counters);
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
        let index_expr = ctx
            .with_expected_type(Type::Int32, |ctx| {
                Self::from_expression_node(index_expr_n.clone(), ctx)
            })
            .maybe_convert_to(Type::Int32, &index_expr_n, ctx.diag, &ctx.symbol_counters);

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
        let values: BTreeMap<SmolStr, Expression> = node
            .ObjectMember()
            .map(|n| {
                let name = identifier_text(&n).unwrap_or_default();
                let field_ty = match &ctx.expected_type {
                    Type::Struct(s) => s.fields.get(&name).cloned().unwrap_or_default(),
                    _ => Type::Invalid,
                };
                let value = ctx.with_expected_type(field_ty, |ctx| {
                    Expression::from_expression_node(n.Expression(), ctx)
                });
                (name, value)
            })
            .collect();
        let ty = Rc::new(Struct::new(
            values.iter().map(|(k, v)| (k.clone(), v.ty())).collect(),
            StructName::None,
        ));
        Expression::Struct { ty, values }
    }

    fn from_array_node(node: syntax_nodes::Array, ctx: &mut LookupCtx) -> Expression {
        let element_expected = match &ctx.expected_type {
            Type::Array(el) => (**el).clone(),
            _ => Type::Invalid,
        };
        let mut values: Vec<Expression> = node
            .Expression()
            .map(|e| {
                ctx.with_expected_type(element_expected.clone(), |ctx| {
                    Expression::from_expression_node(e, ctx)
                })
            })
            .collect();

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
                &ctx.symbol_counters,
            );
        }

        Expression::Array { element_ty, values }
    }

    fn from_string_template_node(
        node: syntax_nodes::StringTemplate,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let mut result = None;
        for n in node.children_with_tokens() {
            let expr = if n.kind() == SyntaxKind::StringLiteral {
                let token = n.as_token().unwrap();
                crate::literals::unescape_string_reporting(Some(token), ctx.diag, token)
                    .map(Self::StringLiteral)
                    .unwrap_or(Self::Invalid)
            } else if n.kind() == SyntaxKind::Expression {
                let node = n.into_node().unwrap();
                let expr = Expression::from_expression_node(node.clone().into(), ctx);
                expr.maybe_convert_to(Type::String, &node, ctx.diag, &ctx.symbol_counters)
            } else {
                continue;
            };
            result = match result {
                Some(result) => Some(Expression::BinaryExpression {
                    lhs: Box::new(result),
                    rhs: Box::new(expr),
                    op: '+',
                }),
                None => Some(expr),
            }
        }
        result.unwrap_or_default()
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
                        // The field defaults must come from the same struct as the name
                        let source = if result.name.is_some() { &result } else { &elem };
                        Type::Struct(Rc::new(Struct {
                            fields,
                            field_defaults: source.field_defaults.clone(),
                            name: source.name.clone(),
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

use i_slint_common::key_codes::{ShiftBehavior, lookup_key_name};

fn with_physical_key_map<R>(
    fun: impl FnOnce(&std::collections::HashMap<&'static str, &'static str>) -> R,
) -> R {
    macro_rules! generate_physical_key_map {
        [ $($name:ident # $code:ident;)* ] => {
            {
                [$( (stringify!($name), stringify!($name)) ),*]
            }
        };
    }

    thread_local! {
        pub static PHYSICAL_KEY_MAP: std::collections::HashMap< &'static str, &'static str>  =
            for_each_physical_keys!(generate_physical_key_map).into_iter().collect();
    }

    PHYSICAL_KEY_MAP.with(fun)
}

fn lookup_physical_key(keycode: &str) -> Option<&'static str> {
    with_physical_key_map(|map| map.get(keycode).copied())
}

/// Return the type that merge two times when they are used in two branch of a condition
///
/// Ideally this could just be Expression::common_target_type_for_type_list, but that function
/// has a bug actually that it tries to convert things that only works for array literal,
/// but doesn't work if we have a type of an array.
/// So try to recurse into struct literal and array literal in expression to only call
/// common_target_type_for_type_list for them, but always keep the type of the array
/// if it is NOT an literal
fn common_expression_type(true_expr: &Expression, false_expr: &Expression) -> Type {
    fn merge_struct(origin: &Struct, other: &Struct) -> Type {
        let mut fields = other.fields.clone();
        fields.extend(origin.fields.iter().map(|(k, v)| (k.clone(), v.clone())));
        Rc::new(Struct::new(fields, StructName::None)).into()
    }

    if let Expression::Struct { ty, values } = true_expr {
        if let Expression::Struct { values: values2, .. } = false_expr {
            let mut fields = BTreeMap::new();
            for (k, v) in values.iter() {
                if let Some(v2) = values2.get(k) {
                    fields.insert(k.clone(), common_expression_type(v, v2));
                } else {
                    fields.insert(k.clone(), v.ty());
                }
            }
            for (k, v) in values2.iter() {
                if !values.contains_key(k) {
                    fields.insert(k.clone(), v.ty());
                }
            }
            return Type::Struct(Rc::new(Struct::new(fields, StructName::None)));
        } else if let Type::Struct(false_ty) = false_expr.ty() {
            return merge_struct(&false_ty, ty);
        }
    } else if let Expression::Struct { ty, .. } = false_expr
        && let Type::Struct(true_ty) = true_expr.ty()
    {
        return merge_struct(&true_ty, ty);
    }

    if let Expression::Array { .. } = true_expr {
        if let Expression::Array { .. } = false_expr {
            // fallback to common_target_type_for_type_list
        } else if let Type::Array(ty) = false_expr.ty() {
            return Type::Array(ty);
        }
    } else if let Expression::Array { .. } = false_expr
        && let Type::Array(ty) = true_expr.ty()
    {
        return Type::Array(ty);
    }

    Expression::common_target_type_for_type_list([true_expr.ty(), false_expr.ty()].into_iter())
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
                if let Some(e) = e
                    && e.lookup(ctx, &first_str).is_some()
                {
                    ctx.diag.push_error(
                        format!(
                            "Unknown unqualified identifier '{0}'. Did you mean '{prefix}.{0}'?",
                            first.text()
                        ),
                        &node,
                    );
                    return None;
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
            expression: mut e @ Expression::RepeaterModelReference { .. },
            ..
        } if matches!(phase, LookupPhase::ResolvingTwoWayBindings) => {
            // The enclosing model expression may not be resolved yet
            // (e.g. when called from `infer_aliases_types`). Skip type
            // checking here; `resolve_two_way_binding` does it later.
            for n in it {
                e = Expression::StructFieldAccess { base: e.into(), name: n.text().into() };
            }
            Some(e.into())
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
        }) = crate::lookup::TypeSpecificLookup.lookup(ctx, &elem.borrow().id)
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
            crate::lookup::check_extra_deprecated(elem, ctx, &prop_name)
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
            let message = format!(
                "The function '{}' is private. Annotate it with 'public' to make it accessible from other components",
                second.text()
            );
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
                ElementType::Global | ElementType::Interface => {
                    let enclosing_type = elem.borrow().enclosing_component.upgrade().unwrap();
                    assert!(enclosing_type.is_global() || enclosing_type.is_interface());
                    format!("'{}'", enclosing_type.id)
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
                if let Some(minus_pos) = next.text().find('-')
                    && base.lookup(ctx, &SmolStr::new(&next.text()[0..minus_pos])).is_some()
                {
                    ctx.diag.push_error(format!("Cannot access the field '{}'. Use space before the '-' if you meant a subtraction", next.text()), &next);
                    return None;
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
                                format!(
                                    " of float. Range expressions are not supported in Slint, but you can use an integer as a model to repeat something multiple time. Eg: `for i in {}`",
                                    next.text()
                                )
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

/// Resolve all two way bindings on `elem`, and finalize the type of any
/// `property foo <=> ...` declared without an explicit type. Run after any
/// enclosing `for` model expression has been resolved.
fn resolve_two_way_bindings_for_element(
    elem: &ElementRc,
    scope: &[ElementRc],
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    // Queued here and applied after the loop, since the iterator holds a
    // borrow on `elem` that blocks `borrow_mut`.
    let mut to_infer: Vec<(SmolStr, Type)> = Vec::new();

    for (prop_name, binding) in &elem.borrow().bindings {
        let mut binding = binding.borrow_mut();
        // The alias node is normally the binding's own (uncompiled) expression. But a
        // global callback may both alias another global's callback and provide a handler:
        // the handler then occupies the expression slot and the alias node lives on the
        // callback declaration, in which case the handler expression must be preserved.
        let twb_from_expression = match binding.expression.ignore_debug_hooks() {
            Expression::Uncompiled(node) => syntax_nodes::TwoWayBinding::new(node.clone()),
            _ => None,
        };
        let twb_node = twb_from_expression
            .clone()
            .or_else(|| elem.borrow().callback_alias_declaration_node(prop_name));
        if let Some(n) = twb_node {
            let node: SyntaxNode = n.clone().into();
            let lhs_lookup = elem.borrow().lookup_property(prop_name);
            if !lhs_lookup.is_valid() {
                // An attempt to resolve this already failed when trying to resolve the property type
                assert!(diag.has_errors());
                continue;
            }
            let mut lookup_ctx = LookupCtx {
                property_name: Some(prop_name.as_str()),
                property_type: lhs_lookup.property_type.clone(),
                expected_type: lhs_lookup.property_type.clone(),
                component_scope: scope,
                diag,
                // Two-way bindings don't generate temporaries; a fresh set is fine.
                symbol_counters: SymbolCounters::shared(),
                arguments: Vec::new(),
                type_register,
                type_loader: None,
                current_token: Some(node.clone().into()),
                local_variables: Vec::new(),
            };

            // Only the alias-only case stores the two-way binding in the expression slot;
            // the combined case must keep its handler expression intact.
            if twb_from_expression.is_some() {
                binding.expression = Expression::Invalid;
            }

            if let Some(twb) = resolve_two_way_binding(n, &mut lookup_ctx) {
                if matches!(lhs_lookup.property_type, Type::InferredProperty) {
                    to_infer.push((prop_name.clone(), twb.ty()));
                }
                let nr = twb.property().cloned();
                binding.two_way_bindings.push(twb);

                let Some(nr) = nr else { continue };
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
                let mut rhs_lookup = nr.element().borrow().lookup_property(nr.name());
                if rhs_lookup.property_type == Type::Invalid {
                    // An attempt to resolve this already failed when trying to resolve the property type
                    assert!(diag.has_errors());
                    continue;
                }
                rhs_lookup.is_local_to_component &= lookup_ctx.is_local_element(&nr.element());

                if !rhs_lookup.is_valid_for_assignment() {
                    match (lhs_lookup.property_visibility, rhs_lookup.property_visibility) {
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
                    } else if rhs_lookup.property_visibility == PropertyVisibility::InOut {
                        diag.push_warning(
                            "Linking input properties to input output properties is deprecated"
                                .into(),
                            &node,
                        );
                        marked_linked_read_only(&nr.element(), nr.name());
                    } else {
                        // This is allowed, but then the rhs must also become read only.
                        marked_linked_read_only(&nr.element(), nr.name());
                    }
                }
            }
        }
    }

    if !to_infer.is_empty() {
        let mut elem_mut = elem.borrow_mut();
        for (prop_name, inferred) in to_infer {
            let decl = elem_mut.property_declarations.get_mut(&prop_name).unwrap();
            if inferred.is_property_type() {
                decl.property_type = inferred;
            } else {
                let type_node = decl.type_node();
                diag.push_error(
                    format!("Could not infer type of property '{prop_name}'"),
                    &type_node,
                );
            }
        }
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
) -> Option<TwoWayBinding> {
    const ERROR_MESSAGE: &str = "The expression in a two way binding must be a property reference";

    let Some(n) = node.Expression().QualifiedName() else {
        ctx.diag.push_error(ERROR_MESSAGE.into(), &node.Expression());
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
        LookupResult::Expression { expression, .. } => {
            fn unwrap_fields(expression: &Expression) -> Option<TwoWayBinding> {
                match expression {
                    Expression::PropertyReference(nr) => Some(nr.clone().into()),
                    Expression::StructFieldAccess { base, name } => {
                        let mut prop = unwrap_fields(base)?;
                        let field_access = match &mut prop {
                            TwoWayBinding::Property { field_access, .. } => field_access,
                            TwoWayBinding::ModelData { field_access, .. } => field_access,
                        };
                        field_access.push(name.clone());
                        Some(prop)
                    }
                    Expression::RepeaterModelReference { element } => {
                        Some(TwoWayBinding::ModelData {
                            repeated_element: element.clone(),
                            field_access: vec![],
                        })
                    }
                    _ => None,
                }
            }
            if let Some(result) = unwrap_fields(&expression) {
                // Walk the `ModelData` field path now: the qualified-name
                // lookup built it without type checks (the row type may not
                // have been known yet). Emits per-field diagnostics and
                // yields the leaf type as `expr_ty`.
                let expr_ty = if let TwoWayBinding::ModelData { repeated_element, field_access } =
                    &result
                {
                    let mut ty =
                        Expression::RepeaterModelReference { element: repeated_element.clone() }
                            .ty();
                    if !matches!(ty, Type::Invalid) {
                        for f in field_access {
                            let next = if let Type::Struct(s) = &ty {
                                s.fields.get(f.as_str()).cloned()
                            } else {
                                None
                            };
                            let Some(next) = next else {
                                ctx.diag.push_error(
                                    format!("Cannot access the field '{f}' of {ty}"),
                                    &node,
                                );
                                return None;
                            };
                            ty = next;
                        }
                    }
                    ty
                } else {
                    result.ty()
                };
                if report_error && expr_ty != ctx.property_type {
                    ctx.diag.push_error(
                        format!(
                            "The property '{}' does not have the same type as the bound expression: {} != {expr_ty}",
                            ctx.property_name.unwrap_or(""),
                            ctx.property_type,
                        ),
                        &node,
                    );
                }
                Some(result)
            } else {
                let kind = match expression {
                    Expression::StructFieldAccess { .. } | Expression::ArrayIndex { .. } => {
                        "Two-way bindings can only target property references"
                    }
                    _ => ERROR_MESSAGE,
                };
                ctx.diag.push_error(kind.into(), &node);
                None
            }
        }
        LookupResult::Callable(LookupResultCallable::Callable(Callable::Callback(n))) => {
            if report_error && n.ty() != ctx.property_type {
                ctx.diag.push_error("Cannot bind to a callback".into(), &node);
                None
            } else {
                Some(n.into())
            }
        }
        LookupResult::Callable(..) => {
            if report_error {
                ctx.diag.push_error("Cannot bind to a function".into(), &node);
            }
            None
        }
        _ => {
            ctx.diag.push_error(ERROR_MESSAGE.into(), &node);
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
    let Some(alias) = b
        .try_borrow()
        .ok()
        .and_then(|b| b.two_way_bindings.first().and_then(|x| x.property()).cloned())
    else {
        return;
    };

    // A non-global element can be instantiated many times, so letting it assign a handler
    // to a singleton global's callback is ambiguous. A global is itself a singleton, so it
    // may implement another global's callback.
    if alias.element().borrow().base_type == ElementType::Global
        && elem_borrow.base_type != ElementType::Global
    {
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
