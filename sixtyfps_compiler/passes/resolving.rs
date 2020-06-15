//! Passes that resolve the property binding expression.
//!
//! Before this pass, all the expression are of type Expression::Uncompiled,
//! and there should no longer be Uncompiled expression after this pass.
//!
//! Most of the code for the resolving actualy lies in the expression_tree module

use crate::diagnostics::Diagnostics;
use crate::expression_tree::*;
use crate::object_tree::*;
use crate::parser::{syntax_nodes, Spanned, SyntaxKind, SyntaxNode, SyntaxNodeEx};
use crate::typeregister::{Type, TypeRegister};
use core::str::FromStr;
use std::rc::Rc;

pub fn resolve_expressions(doc: &Document, diag: &mut Diagnostics, tr: &mut TypeRegister) {
    for component in &doc.inner_components {
        recurse_elem(&component.root_element, &mut |elem| {
            // We are taking the binding to mutate them, as we cannot keep a borrow of the element
            // during the creation of the expression (we need to be able to borrow the Element to do lookups)
            // the `bindings` will be reset later
            let mut bindings = std::mem::take(&mut elem.borrow_mut().bindings);
            for (prop, expr) in &mut bindings {
                if let Expression::Uncompiled(node) = expr {
                    let mut lookup_ctx = LookupCtx {
                        tr,
                        property_type: elem.borrow().lookup_property(&*prop),
                        component: component.clone(),
                        diag,
                    };

                    let new_expr = if matches!(lookup_ctx.property_type, Type::Signal) {
                        //FIXME: proper signal suport (node is a codeblock)
                        node.child_node(SyntaxKind::Expression)
                            .map(|en| Expression::from_expression_node(en, &mut lookup_ctx))
                            .unwrap_or(Expression::Invalid)
                    } else {
                        Expression::from_binding_expression_node(node.clone(), &mut lookup_ctx)
                    };
                    *expr = new_expr;
                }
            }
            elem.borrow_mut().bindings = bindings;
            let mut repeated = elem.borrow_mut().repeated.take();
            if let Some(r) = &mut repeated {
                if let Expression::Uncompiled(node) = &mut r.model {
                    let mut lookup_ctx = LookupCtx {
                        tr,
                        property_type: Type::Invalid, // FIXME: that should be a model
                        component: component.clone(),
                        diag,
                    };
                    r.model =
                        Expression::from_binding_expression_node(node.clone(), &mut lookup_ctx)
                }
            }
            elem.borrow_mut().repeated = repeated;
        })
    }
}

/// Contains information which allow to lookup identifier in expressions
struct LookupCtx<'a> {
    #[allow(dead_code)]
    /// The type register
    tr: &'a TypeRegister,
    /// the type of the property for which this expression refers.
    /// (some property come in the scope)
    property_type: Type,

    /// document_root
    component: Rc<Component>,

    /// Somewhere to report diagnostics
    diag: &'a mut Diagnostics,
}

impl Expression {
    fn from_binding_expression_node(node: SyntaxNode, ctx: &mut LookupCtx) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::BindingExpression);
        let e = node
            .child_node(SyntaxKind::Expression)
            .map(|n| Self::from_expression_node(n, ctx))
            .or_else(|| {
                node.child_node(SyntaxKind::CodeBlock).map(|c| Self::from_codeblock_node(c, ctx))
            })
            .unwrap_or(Self::Invalid);
        e.maybe_convert_to(ctx.property_type.clone(), &node, &mut ctx.diag)
    }

    fn from_codeblock_node(node: SyntaxNode, ctx: &mut LookupCtx) -> Expression {
        debug_assert_eq!(node.kind(), SyntaxKind::CodeBlock);
        Expression::CodeBlock(
            node.children()
                .filter(|n| n.kind() == SyntaxKind::Expression)
                .map(|n| Self::from_expression_node(n, ctx))
                .collect(),
        )
    }

    fn from_expression_node(node: SyntaxNode, ctx: &mut LookupCtx) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Expression);
        node.child_node(SyntaxKind::Expression)
            .map(|n| Self::from_expression_node(n, ctx))
            .or_else(|| {
                node.child_node(SyntaxKind::BangExpression)
                    .map(|n| Self::from_bang_expresion_node(n, ctx))
            })
            .or_else(|| {
                node.child_node(SyntaxKind::QualifiedName)
                    .map(|s| Self::from_qualified_name_node(s, ctx))
            })
            .or_else(|| {
                node.child_text(SyntaxKind::StringLiteral).map(|s| {
                    unescape_string(&s).map(Self::StringLiteral).unwrap_or_else(|| {
                        ctx.diag.push_error("Cannot parse string literal".into(), node.span());
                        Self::Invalid
                    })
                })
            })
            .or_else(|| {
                node.child_text(SyntaxKind::NumberLiteral).map(|s| {
                    f64::from_str(&s).ok().map(Self::NumberLiteral).unwrap_or_else(|| {
                        ctx.diag.push_error("Cannot parse number literal".into(), node.span());
                        Self::Invalid
                    })
                })
            })
            .or_else(|| {
                node.child_text(SyntaxKind::ColorLiteral).map(|s| {
                    parse_color_literal(&s)
                        .map(|i| Expression::Cast {
                            from: Box::new(Expression::NumberLiteral(i as _)),
                            to: Type::Color,
                        })
                        .unwrap_or_else(|| {
                            ctx.diag.push_error("Invalid color literal".into(), node.span());
                            Self::Invalid
                        })
                })
            })
            .or_else(|| {
                node.child_node(SyntaxKind::FunctionCallExpression).map(|n| {
                    Expression::FunctionCall {
                        function: Box::new(
                            n.child_node(SyntaxKind::Expression)
                                .map(|n| Self::from_expression_node(n, ctx))
                                .unwrap_or(Expression::Invalid),
                        ),
                    }
                })
            })
            .or_else(|| {
                node.child_node(SyntaxKind::SelfAssignment)
                    .map(|n| Self::from_self_assignement_node(n.into(), ctx))
            })
            .or_else(|| {
                node.child_node(SyntaxKind::ConditionalExpression)
                    .map(|n| Self::from_conditional_expression_node(n.into(), ctx))
            })
            .unwrap_or(Self::Invalid)
    }

    fn from_bang_expresion_node(node: SyntaxNode, ctx: &mut LookupCtx) -> Self {
        match node.child_text(SyntaxKind::Identifier).as_ref().map(|x| x.as_str()) {
            None => {
                debug_assert!(false, "the parser should not allow that");
                ctx.diag.push_error("Missing bang keyword".into(), node.span());
                return Self::Invalid;
            }
            Some("img") => {
                // FIXME: we probably need a better syntax and make this at another level.
                let s = match node
                    .child_node(SyntaxKind::Expression)
                    .map_or(Self::Invalid, |n| Self::from_expression_node(n, ctx))
                {
                    Expression::StringLiteral(p) => p,
                    _ => {
                        ctx.diag.push_error(
                            "img! Must be followed by a valid path".into(),
                            node.span(),
                        );
                        return Self::Invalid;
                    }
                };

                let absolute_source_path = {
                    let path = std::path::Path::new(&s);

                    if path.is_absolute() {
                        s
                    } else {
                        let path = ctx.diag.path(node.span()).parent().unwrap().join(path);
                        if path.is_absolute() {
                            path.to_string_lossy().to_string()
                        } else {
                            std::env::current_dir()
                                .unwrap()
                                .join(path)
                                .to_string_lossy()
                                .to_string()
                        }
                    }
                };

                Expression::ResourceReference { absolute_source_path }
            }
            Some(x) => {
                ctx.diag.push_error(format!("Unknown bang keyword `{}`", x), node.span());
                return Self::Invalid;
            }
        }
    }

    /// Perform the lookup
    fn from_qualified_name_node(node: SyntaxNode, ctx: &mut LookupCtx) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::QualifiedName);

        let mut it = node.children_with_tokens().filter(|n| n.kind() == SyntaxKind::Identifier);

        let first = if let Some(first) = it.next() {
            first.into_token().unwrap()
        } else {
            // There must be at least one member (parser should ensure that)
            debug_assert!(ctx.diag.has_error());
            return Self::Invalid;
        };

        let s = first.text().as_str();

        let property = ctx.component.root_element.borrow().lookup_property(s);
        if property.is_property_type() {
            if let Some(x) = it.next() {
                ctx.diag.push_error(
                    "Cannot access fields of property".into(),
                    x.into_token().unwrap().span(),
                )
            }
            return Self::PropertyReference(NamedReference {
                element: Rc::downgrade(&ctx.component.root_element),
                name: s.to_string(),
            });
        } else if matches!(property, Type::Signal) {
            if let Some(x) = it.next() {
                ctx.diag.push_error(
                    "Cannot access fields of signal".into(),
                    x.into_token().unwrap().span(),
                )
            }
            return Self::SignalReference(NamedReference {
                element: Rc::downgrade(&ctx.component.root_element),
                name: s.to_string(),
            });
        } else if property.is_object_type() {
            todo!("Continue lookling up");
        }

        if let Some(elem) = ctx.component.find_element_by_id(s) {
            let prop_name = if let Some(second) = it.next() {
                second.into_token().unwrap()
            } else {
                ctx.diag.push_error("Cannot take reference of an element".into(), node.span());
                return Self::Invalid;
            };

            let p = elem.borrow().lookup_property(prop_name.text().as_str());
            if p.is_property_type() {
                if let Some(x) = it.next() {
                    ctx.diag.push_error(
                        "Cannot access fields of property".into(),
                        x.into_token().unwrap().span(),
                    );
                    return Self::Invalid;
                }
                return Self::PropertyReference(NamedReference {
                    element: Rc::downgrade(&elem),
                    name: prop_name.text().to_string(),
                });
            } else if matches!(p, Type::Signal) {
                if let Some(x) = it.next() {
                    ctx.diag.push_error(
                        "Cannot access fields of signal".into(),
                        x.into_token().unwrap().span(),
                    )
                }
                return Self::SignalReference(NamedReference {
                    element: Rc::downgrade(&elem),
                    name: prop_name.to_string(),
                });
            } else {
                ctx.diag.push_error(
                    format!("Cannot access property '{}'", prop_name),
                    prop_name.span(),
                );
                return Self::Invalid;
            }
        }

        if it.next().is_some() {
            ctx.diag.push_error(format!("Cannot access id '{}'", s), node.span());
            return Expression::Invalid;
        }

        if matches!(ctx.property_type, Type::Color) {
            let value: Option<u32> = match s {
                "blue" => Some(0xff0000ff),
                "red" => Some(0xffff0000),
                "green" => Some(0xff00ff00),
                "yellow" => Some(0xffffff00),
                "black" => Some(0xff000000),
                "white" => Some(0xffffffff),
                _ => None,
            };
            if let Some(value) = value {
                return Expression::Cast {
                    from: Box::new(Expression::NumberLiteral(value as f64)),
                    to: Type::Color,
                };
            }
        }

        ctx.diag.push_error(format!("Unknown unqualified identifier '{}'", s), node.span());

        Self::Invalid
    }

    fn from_self_assignement_node(
        node: syntax_nodes::SelfAssignment,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let (lhs_n, rhs_n) = node.Expression();
        let lhs = Self::from_expression_node(lhs_n.into(), ctx);
        if !matches!(lhs, Expression::PropertyReference{..}) {
            ctx.diag
                .push_error("Self assignement need to be done on a property".into(), node.span());
        }
        let rhs = Self::from_expression_node(rhs_n.clone().into(), ctx).maybe_convert_to(
            lhs.ty(),
            &rhs_n.into(),
            &mut ctx.diag,
        );
        Expression::SelfAssignment {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            op: None
                .or(node.child_token(SyntaxKind::PlusEqual).and(Some('+')))
                .or(node.child_token(SyntaxKind::MinusEqual).and(Some('-')))
                .or(node.child_token(SyntaxKind::StarEqual).and(Some('*')))
                .or(node.child_token(SyntaxKind::DivEqual).and(Some('/')))
                .unwrap_or('_'),
        }
    }

    fn from_conditional_expression_node(
        node: syntax_nodes::ConditionalExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let (condition_n, true_expr_n, false_expr_n) = node.Expression();
        // FIXME: we should we add bool to the context
        let condition = Self::from_expression_node(condition_n.clone().into(), ctx)
            .maybe_convert_to(Type::Bool, &condition_n.into(), &mut ctx.diag);
        let mut true_expr = Self::from_expression_node(true_expr_n.clone().into(), ctx);
        let mut false_expr = Self::from_expression_node(false_expr_n.clone().into(), ctx);
        let (true_ty, false_ty) = (true_expr.ty(), false_expr.ty());
        if true_ty != false_ty {
            if false_ty.can_convert(&true_ty) {
                false_expr =
                    false_expr.maybe_convert_to(true_ty, &false_expr_n.into(), &mut ctx.diag);
            } else {
                true_expr =
                    true_expr.maybe_convert_to(false_ty, &true_expr_n.into(), &mut ctx.diag);
            }
        }
        Expression::Condition {
            condition: Box::new(condition),
            true_expr: Box::new(true_expr),
            false_expr: Box::new(false_expr),
        }
    }
}

fn parse_color_literal(s: &str) -> Option<u32> {
    if !s.starts_with("#") {
        return None;
    }
    if !s.is_ascii() {
        return None;
    }
    let s = &s[1..];
    let (r, g, b, a) = match s.len() {
        3 => (
            u8::from_str_radix(&s[0..=0], 16).ok()? * 0x11,
            u8::from_str_radix(&s[1..=1], 16).ok()? * 0x11,
            u8::from_str_radix(&s[2..=2], 16).ok()? * 0x11,
            255u8,
        ),
        4 => (
            u8::from_str_radix(&s[0..=0], 16).ok()? * 0x11,
            u8::from_str_radix(&s[1..=1], 16).ok()? * 0x11,
            u8::from_str_radix(&s[2..=2], 16).ok()? * 0x11,
            u8::from_str_radix(&s[3..=3], 16).ok()? * 0x11,
        ),
        6 => (
            u8::from_str_radix(&s[0..2], 16).ok()?,
            u8::from_str_radix(&s[2..4], 16).ok()?,
            u8::from_str_radix(&s[4..6], 16).ok()?,
            255u8,
        ),
        8 => (
            u8::from_str_radix(&s[0..2], 16).ok()?,
            u8::from_str_radix(&s[2..4], 16).ok()?,
            u8::from_str_radix(&s[4..6], 16).ok()?,
            u8::from_str_radix(&s[6..8], 16).ok()?,
        ),
        _ => return None,
    };
    Some((a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | (b as u32) << 0)
}

#[test]
fn test_parse_color_literal() {
    assert_eq!(parse_color_literal("#abc"), Some(0xffaabbcc));
    assert_eq!(parse_color_literal("#ABC"), Some(0xffaabbcc));
    assert_eq!(parse_color_literal("#AbC"), Some(0xffaabbcc));
    assert_eq!(parse_color_literal("#AbCd"), Some(0xddaabbcc));
    assert_eq!(parse_color_literal("#01234567"), Some(0x67012345));
    assert_eq!(parse_color_literal("#012345"), Some(0xff012345));
    assert_eq!(parse_color_literal("_01234567"), None);
    assert_eq!(parse_color_literal("→↓←"), None);
    assert_eq!(parse_color_literal("#→↓←"), None);
    assert_eq!(parse_color_literal("#1234567890"), None);
}

fn unescape_string(string: &str) -> Option<String> {
    if !string.starts_with('"') || !string.ends_with('"') {
        return None;
    }
    let string = &string[1..(string.len() - 1)];
    // TODO: remove slashes
    return Some(string.into());
}
