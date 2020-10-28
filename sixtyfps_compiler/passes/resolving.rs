/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Passes that resolve the property binding expression.
//!
//! Before this pass, all the expression are of type Expression::Uncompiled,
//! and there should no longer be Uncompiled expression after this pass.
//!
//! Most of the code for the resolving actualy lies in the expression_tree module

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::*;
use crate::langtype::Type;
use crate::object_tree::*;
use crate::parser::{identifier_text, syntax_nodes, SyntaxKind, SyntaxNodeWithSourceFile};
use crate::typeregister::TypeRegister;
use by_address::ByAddress;
use std::{collections::HashMap, collections::HashSet, rc::Rc};

#[derive(Default)]
/// Helper type to trace through a document and locate all the used components.
struct ComponentCollection {
    components_used: HashSet<ByAddress<Rc<Component>>>,
}

impl ComponentCollection {
    fn add_document(&mut self, doc: &Document) {
        doc.inner_components.iter().for_each(|component| self.add_component(component));
    }
    fn add_component(&mut self, component: &Rc<Component>) {
        let component_key = ByAddress(component.clone());
        match self.components_used.get(&component_key) {
            Some(_) => return,
            None => {
                self.components_used.insert(component_key);
                self.add_types_used_in_components(component);
            }
        };
    }
    fn add_types_used_in_components(&mut self, component: &Rc<Component>) {
        recurse_elem(&component.root_element, &(), &mut |element: &ElementRc, _| {
            self.add_type(&element.borrow().base_type);
            // ### traverse more
        });
    }
    fn add_type(&mut self, ty: &Type) {
        if let Type::Component(component) = ty {
            self.add_component(component);
        }
    }

    fn iter(&self) -> impl Iterator<Item = &Rc<Component>> {
        self.components_used.iter().map(|byaddr_key| &**byaddr_key)
    }
}

/// This represeresent a scope for the Component, where Component is the repeated component, but
/// does not represent a component in the .60 file
#[derive(Clone)]
struct ComponentScope(Vec<ElementRc>);

fn resolve_expression(
    expr: &mut Expression,
    property_name: Option<&str>,
    property_type: Type,
    scope: &ComponentScope,
    type_register: &TypeRegister,
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
        };

        let new_expr = match node.kind() {
            SyntaxKind::SignalConnection => {
                //FIXME: proper signal suport (node is a codeblock)
                Expression::from_signal_connection(node.clone().into(), &mut lookup_ctx)
            }
            SyntaxKind::Expression => {
                //FIXME again: this happen for non-binding expression (i.e: model)
                Expression::from_expression_node(node.clone().into(), &mut lookup_ctx)
                    .maybe_convert_to(
                        lookup_ctx.property_type.clone(),
                        lookup_ctx.symmetric_parent_property(),
                        node,
                        diag,
                    )
            }
            SyntaxKind::BindingExpression => {
                Expression::from_binding_expression_node(node.clone(), &mut lookup_ctx)
            }
            SyntaxKind::TwoWayBinding => {
                Expression::from_two_way_binding(node.clone().into(), &mut lookup_ctx)
            }
            _ => {
                debug_assert!(diag.has_error());
                Expression::Invalid
            }
        };
        *expr = new_expr;
    }
}

pub fn resolve_expressions(doc: &Document, diag: &mut BuildDiagnostics) {
    let mut all_components = ComponentCollection::default();
    all_components.add_document(&doc);
    for component in all_components.iter() {
        let scope = ComponentScope(vec![component.root_element.clone()]);

        recurse_elem(&component.root_element, &scope, &mut |elem, scope| {
            let mut new_scope = scope.clone();
            let mut is_repeated = elem.borrow().repeated.is_some();
            if is_repeated {
                new_scope.0.push(elem.clone())
            }
            new_scope.0.push(elem.clone());
            visit_element_expressions(elem, |expr, property_name, property_type| {
                if is_repeated {
                    // The first expression is always the model and it needs to be resolved with the parent scope
                    debug_assert!(elem.borrow().repeated.as_ref().is_none()); // should be none because it is taken by the visit_element_expressions function
                    resolve_expression(
                        expr,
                        property_name,
                        property_type(),
                        scope,
                        &doc.local_registry,
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
                        diag,
                    )
                }
            });
            new_scope.0.pop();
            new_scope
        })
    }
}

/// Contains information which allow to lookup identifier in expressions
pub struct LookupCtx<'a> {
    /// the name of the property for which this expression refers.
    property_name: Option<&'a str>,

    /// the type of the property for which this expression refers.
    /// (some property come in the scope)
    property_type: Type,

    /// Here is the stack in which id applies
    component_scope: &'a [ElementRc],

    /// Somewhere to report diagnostics
    diag: &'a mut BuildDiagnostics,

    /// The name of the arguments of the signal or function
    arguments: Vec<String>,

    /// The type register in which to look for Globals
    type_register: &'a TypeRegister,
}

impl<'a> LookupCtx<'a> {
    pub fn symmetric_parent_property(&self) -> Option<(Type, NamedReference)> {
        self.component_scope.last().and_then(find_parent_element).and_then(|parent| {
            self.property_name.and_then(|name| {
                let ty = parent.borrow().lookup_property(name);
                if ty.is_property_type() {
                    Some((
                        ty,
                        NamedReference { element: Rc::downgrade(&parent), name: name.to_string() },
                    ))
                } else {
                    None
                }
            })
        })
    }
}

fn find_element_by_id(roots: &[ElementRc], name: &str) -> Option<ElementRc> {
    for e in roots.iter().rev() {
        if e.borrow().id == name {
            return Some(e.clone());
        }
        for x in &e.borrow().children {
            if x.borrow().repeated.is_some() {
                continue;
            }
            if let Some(x) = find_element_by_id(&[x.clone()], name) {
                return Some(x);
            }
        }
    }
    None
}

/// Find the parent element to a given element.
/// (since there is no parent mapping we need to fo an exhaustive search)
fn find_parent_element(e: &ElementRc) -> Option<ElementRc> {
    fn recurse(base: &ElementRc, e: &ElementRc) -> Option<ElementRc> {
        for child in &base.borrow().children {
            if Rc::ptr_eq(child, e) {
                return Some(base.clone());
            }
            if let Some(x) = recurse(child, e) {
                return Some(x);
            }
        }
        None
    }

    let root = e.borrow().enclosing_component.upgrade().unwrap().root_element.clone();
    if Rc::ptr_eq(&root, e) {
        return None;
    }
    recurse(&root, e)
}

impl Expression {
    fn from_binding_expression_node(node: SyntaxNodeWithSourceFile, ctx: &mut LookupCtx) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::BindingExpression);
        let e = node
            .child_node(SyntaxKind::Expression)
            .map(|n| Self::from_expression_node(n.into(), ctx))
            .or_else(|| {
                node.child_node(SyntaxKind::CodeBlock)
                    .map(|c| Self::from_codeblock_node(c.into(), ctx))
            })
            .unwrap_or(Self::Invalid);
        e.maybe_convert_to(
            ctx.property_type.clone(),
            ctx.symmetric_parent_property(),
            &node,
            &mut ctx.diag,
        )
    }

    fn from_codeblock_node(node: syntax_nodes::CodeBlock, ctx: &mut LookupCtx) -> Expression {
        debug_assert_eq!(node.kind(), SyntaxKind::CodeBlock);
        Expression::CodeBlock(
            node.children()
                .filter(|n| n.kind() == SyntaxKind::Expression)
                .map(|n| Self::from_expression_node(n.into(), ctx))
                .collect(),
        )
    }

    fn from_signal_connection(
        node: syntax_nodes::SignalConnection,
        ctx: &mut LookupCtx,
    ) -> Expression {
        ctx.arguments =
            node.DeclaredIdentifier().map(|x| identifier_text(&x).unwrap_or_default()).collect();
        Self::from_codeblock_node(node.CodeBlock(), ctx)
    }

    fn from_two_way_binding(node: syntax_nodes::TwoWayBinding, ctx: &mut LookupCtx) -> Expression {
        let e = Self::from_expression_node(node.Expression(), ctx);
        let ty = e.ty();
        match e {
            Expression::PropertyReference(n) => {
                if ty != ctx.property_type {
                    ctx.diag.push_error(
                        "The property does not have the same type as the bound property".into(),
                        &node,
                    );
                }
                Expression::TwoWayBinding(n, None)
            }
            _ => {
                ctx.diag.push_error(
                    "The expression in a two way binding must be a property reference".into(),
                    &node,
                );
                e
            }
        }
    }

    fn from_expression_node(node: syntax_nodes::Expression, ctx: &mut LookupCtx) -> Self {
        node.Expression()
            .map(|n| Self::from_expression_node(n, ctx))
            .or_else(|| {
                node.BangExpression().map(|n| Self::from_bang_expression_node(n.into(), ctx))
            })
            .or_else(|| node.QualifiedName().map(|s| Self::from_qualified_name_node(s.into(), ctx)))
            .or_else(|| {
                node.child_text(SyntaxKind::StringLiteral).map(|s| {
                    unescape_string(&s).map(Self::StringLiteral).unwrap_or_else(|| {
                        ctx.diag.push_error("Cannot parse string literal".into(), &node);
                        Self::Invalid
                    })
                })
            })
            .or_else(|| {
                node.child_text(SyntaxKind::NumberLiteral)
                    .map(parse_number_literal)
                    .transpose()
                    .unwrap_or_else(|e| {
                        ctx.diag.push_error(e, &node);
                        Some(Self::Invalid)
                    })
            })
            .or_else(|| {
                node.child_text(SyntaxKind::ColorLiteral).map(|s| {
                    parse_color_literal(&s)
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
            .or_else(|| node.SelfAssignment().map(|n| Self::from_self_assignement_node(n, ctx)))
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
            .unwrap_or(Self::Invalid)
    }

    fn from_bang_expression_node(node: SyntaxNodeWithSourceFile, ctx: &mut LookupCtx) -> Self {
        match identifier_text(&node).as_ref().map(|x| x.as_str()) {
            None => {
                debug_assert!(false, "the parser should not allow that");
                ctx.diag.push_error("Missing bang keyword".into(), &node);
                return Self::Invalid;
            }
            Some("img") => {
                // FIXME: we probably need a better syntax and make this at another level.
                let s = match node
                    .child_node(SyntaxKind::Expression)
                    .map_or(Self::Invalid, |n| Self::from_expression_node(n.into(), ctx))
                {
                    Expression::StringLiteral(p) => p,
                    _ => {
                        ctx.diag.push_error("img! Must be followed by a valid path".into(), &node);
                        return Self::Invalid;
                    }
                };

                let absolute_source_path = {
                    let path = std::path::Path::new(&s);

                    if path.is_absolute() || s.starts_with("http://") || s.starts_with("https://") {
                        s
                    } else {
                        let path = node
                            .source_file
                            .unwrap_or_default()
                            .parent()
                            .map(|b| b.join(path))
                            .unwrap_or_else(|| path.to_owned());
                        if path.is_absolute() {
                            path.to_string_lossy().to_string()
                        } else {
                            std::env::current_dir()
                                .map(|b| b.join(&path))
                                .unwrap_or(path)
                                .to_string_lossy()
                                .to_string()
                        }
                    }
                };

                Expression::ResourceReference { absolute_source_path }
            }
            Some(x) => {
                ctx.diag.push_error(format!("Unknown bang keyword `{}`", x), &node);
                return Self::Invalid;
            }
        }
    }

    /// Perform the lookup
    fn from_qualified_name_node(node: SyntaxNodeWithSourceFile, ctx: &mut LookupCtx) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::QualifiedName);

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

        let first_str = crate::parser::normalize_identifier(first.text().as_str());

        if let Some(index) = ctx.arguments.iter().position(|x| x == &first_str) {
            let ty = match &ctx.property_type {
                Type::Signal { args } | Type::Function { args, .. } => args[index].clone(),
                _ => panic!("There should only be argument within functions or signal"),
            };
            let e = Expression::FunctionParameterReference { index, ty };
            return maybe_lookup_object(e, it, ctx);
        }

        let elem_opt = match first_str.as_str() {
            "self" => ctx.component_scope.last().cloned(),
            "parent" => ctx.component_scope.last().and_then(find_parent_element),
            "true" => return Self::BoolLiteral(true),
            "false" => return Self::BoolLiteral(false),
            _ => find_element_by_id(ctx.component_scope, &first_str).or_else(|| {
                if let Type::Component(c) = ctx.type_register.lookup(&first_str) {
                    if c.is_global() {
                        return Some(c.root_element.clone());
                    }
                }
                None
            }),
        };

        if let Some(elem) = elem_opt {
            return continue_lookup_within_element(&elem, &mut it, node, ctx);
        }

        for elem in ctx.component_scope.iter().rev() {
            if let Some(repeated) = &elem.borrow().repeated {
                if first_str == repeated.index_id {
                    return Expression::RepeaterIndexReference { element: Rc::downgrade(elem) };
                } else if first_str == repeated.model_data_id {
                    let base = Expression::RepeaterModelReference { element: Rc::downgrade(elem) };
                    return maybe_lookup_object(base, it, ctx);
                }
            }

            let property = elem.borrow().lookup_property(&first_str);
            if property.is_property_type() {
                let prop = Self::PropertyReference(NamedReference {
                    element: Rc::downgrade(&elem),
                    name: first_str.to_string(),
                });
                return maybe_lookup_object(prop, it, ctx);
            } else if matches!(property, Type::Signal{..}) {
                if let Some(x) = it.next() {
                    ctx.diag.push_error("Cannot access fields of signal".into(), &x)
                }
                return Self::SignalReference(NamedReference {
                    element: Rc::downgrade(&elem),
                    name: first_str.to_string(),
                });
            } else if property.is_object_type() {
                todo!("Continue lookling up");
            }
        }

        if it.next().is_some() {
            ctx.diag.push_error(format!("Cannot access id '{}'", first_str), &node);
            return Expression::Invalid;
        }

        match &ctx.property_type {
            Type::Color => {
                if let Some(c) = css_color_parser2::NAMED_COLORS.get(first_str.as_str()) {
                    let value = ((c.a as u32 * 255) << 24)
                        | ((c.r as u32) << 16)
                        | ((c.g as u32) << 8)
                        | (c.b as u32);
                    return Expression::Cast {
                        from: Box::new(Expression::NumberLiteral(value as f64, Unit::None)),
                        to: Type::Color,
                    };
                }
            }
            Type::Easing => {
                // These value are coming from CSSn with - replaced by _
                let value = match first_str.as_str() {
                    "linear" => Some(EasingCurve::Linear),
                    "ease" => Some(EasingCurve::CubicBezier(0.25, 0.1, 0.25, 1.0)),
                    "ease_in" => Some(EasingCurve::CubicBezier(0.42, 0.0, 1.0, 1.0)),
                    "ease_in_out" => Some(EasingCurve::CubicBezier(0.42, 0.0, 0.58, 1.0)),
                    "ease_out" => Some(EasingCurve::CubicBezier(0.0, 0.0, 0.58, 1.0)),
                    "cubic_bezier" => todo!("Not yet implemented"),
                    _ => None,
                };
                if let Some(curve) = value {
                    return Expression::EasingCurve(curve);
                }
            }
            Type::Enumeration(enumeration) => {
                if let Some(value) = enumeration.clone().try_value_from_string(&first_str) {
                    return Expression::EnumerationValue(value);
                }
            }
            _ => {}
        }

        // Builtin functions  FIXME: handle that in a registery or something
        if first_str == "debug" {
            return Expression::BuiltinFunctionReference(BuiltinFunction::Debug);
        }

        // Attempt to recover if the user wanted to write "-"
        if let Some(minus_pos) = first.text().find('-') {
            let report_minus_error = |ctx: &mut LookupCtx| {
                ctx.diag.push_error(format!("Unknown unqualified identifier '{}'. Use space before the '-' if you meant a substraction.", first.text()), &node);
            };
            let first_str = &first.text()[0..minus_pos];
            for elem in ctx.component_scope.iter().rev() {
                if let Some(repeated) = &elem.borrow().repeated {
                    if first_str == repeated.index_id || first_str == repeated.model_data_id {
                        report_minus_error(ctx);
                        return Expression::Invalid;
                    }
                }

                let property = elem.borrow().lookup_property(&first_str);
                if property.is_property_type() {
                    report_minus_error(ctx);
                    return Expression::Invalid;
                }
            }
        }

        ctx.diag.push_error(format!("Unknown unqualified identifier '{}'", first.text()), &node);

        Self::Invalid
    }

    fn from_function_call_node(
        node: syntax_nodes::FunctionCallExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let mut sub_expr =
            node.Expression().map(|n| (Self::from_expression_node(n.clone(), ctx), n.0));

        let mut arguments = Vec::new();

        let function = sub_expr.next().map_or(Expression::Invalid, |e| e.0);
        let function = if let Expression::MemberFunction { base, base_node, member } = function {
            arguments.push((*base, base_node));
            member
        } else {
            Box::new(function)
        };

        arguments.extend(sub_expr);

        let arguments = match function.ty() {
            Type::Function { args, .. } | Type::Signal { args } => {
                if arguments.len() != args.len() {
                    ctx.diag.push_error(
                        format!(
                            "The signal or function expects {} arguments, but {} are provided",
                            args.len(),
                            arguments.len()
                        ),
                        &node,
                    );
                    arguments.into_iter().map(|x| x.0).collect()
                } else {
                    arguments
                        .into_iter()
                        .zip(args.iter())
                        .map(|((e, node), ty)| {
                            e.maybe_convert_to(ty.clone(), None, &node, &mut ctx.diag)
                        })
                        .collect()
                }
            }
            _ => {
                ctx.diag.push_error("The expression is not a function".into(), &node);
                arguments.into_iter().map(|x| x.0).collect()
            }
        };

        Expression::FunctionCall { function, arguments }
    }

    fn from_self_assignement_node(
        node: syntax_nodes::SelfAssignment,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let (lhs_n, rhs_n) = node.Expression();
        let lhs = Self::from_expression_node(lhs_n.into(), ctx);
        let op = None
            .or(node.child_token(SyntaxKind::PlusEqual).and(Some('+')))
            .or(node.child_token(SyntaxKind::MinusEqual).and(Some('-')))
            .or(node.child_token(SyntaxKind::StarEqual).and(Some('*')))
            .or(node.child_token(SyntaxKind::DivEqual).and(Some('/')))
            .or(node.child_token(SyntaxKind::Equal).and(Some('=')))
            .unwrap_or('_');
        if !lhs.is_rw() && lhs.ty() != Type::Invalid {
            ctx.diag.push_error(
                format!(
                    "{} need to be done on a property",
                    if op == '=' { "Assignement" } else { "Self assignement" }
                ),
                &node,
            );
        }
        let rhs = Self::from_expression_node(rhs_n.clone().into(), ctx).maybe_convert_to(
            lhs.ty(),
            None,
            &rhs_n,
            &mut ctx.diag,
        );
        Expression::SelfAssignment { lhs: Box::new(lhs), rhs: Box::new(rhs), op }
    }

    fn from_binary_expression_node(
        node: syntax_nodes::BinaryExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let op = None
            .or(node.child_token(SyntaxKind::Plus).and(Some('+')))
            .or(node.child_token(SyntaxKind::Minus).and(Some('-')))
            .or(node.child_token(SyntaxKind::Star).and(Some('*')))
            .or(node.child_token(SyntaxKind::Div).and(Some('/')))
            .or(node.child_token(SyntaxKind::LessEqual).and(Some('≤')))
            .or(node.child_token(SyntaxKind::GreaterEqual).and(Some('≥')))
            .or(node.child_token(SyntaxKind::LAngle).and(Some('<')))
            .or(node.child_token(SyntaxKind::RAngle).and(Some('>')))
            .or(node.child_token(SyntaxKind::EqualEqual).and(Some('=')))
            .or(node.child_token(SyntaxKind::NotEqual).and(Some('!')))
            .or(node.child_token(SyntaxKind::AndAnd).and(Some('&')))
            .or(node.child_token(SyntaxKind::OrOr).and(Some('|')))
            .unwrap_or('_');

        let (lhs_n, rhs_n) = node.Expression();
        let lhs = Self::from_expression_node(lhs_n.clone().into(), ctx);
        let rhs = Self::from_expression_node(rhs_n.clone().into(), ctx);

        let expected_ty = match operator_class(op) {
            OperatorClass::ComparisonOp => {
                let (lhs_ty, rhs_ty) = (lhs.ty(), rhs.ty());
                if rhs_ty.can_convert(&lhs_ty) {
                    lhs_ty
                } else {
                    rhs_ty
                }
            }
            OperatorClass::LogicalOp => Type::Bool,
            OperatorClass::ArithmeticOp => {
                macro_rules! unit_operations {
                    ($($unit:ident)*) => {
                        match (op, lhs.ty(), rhs.ty()) {
                            ('+', Type::String, _) => Type::String,
                            ('+', _, Type::String) => Type::String,

                            $(
                                ('+', Type::$unit, _) => Type::$unit,
                                ('-', Type::$unit, _) => Type::$unit,
                                ('*', Type::$unit, _) => {
                                    return Expression::BinaryExpression {
                                        lhs: Box::new(lhs),
                                        rhs: Box::new(rhs.maybe_convert_to(
                                            Type::Float32,
                                            None,
                                            &lhs_n,
                                            &mut ctx.diag,
                                        )),
                                        op,
                                    }
                                }
                                ('*', _, Type::$unit) => {
                                    return Expression::BinaryExpression {
                                        lhs: Box::new(lhs.maybe_convert_to(
                                            Type::Float32,
                                            None,
                                            &lhs_n,
                                            &mut ctx.diag,
                                        )),
                                        rhs: Box::new(rhs),
                                        op,
                                    }
                                }
                                ('/', Type::$unit, Type::$unit) => {
                                    return Expression::BinaryExpression {
                                        lhs: Box::new(lhs),
                                        rhs: Box::new(rhs),
                                        op,
                                    }
                                }
                                ('/', Type::$unit, _) => {
                                    return Expression::BinaryExpression {
                                        lhs: Box::new(lhs),
                                        rhs: Box::new(rhs.maybe_convert_to(
                                            Type::Float32,
                                            None,
                                            &lhs_n,
                                            &mut ctx.diag,
                                        )),
                                        op,
                                    }
                                }
                            )*
                            _ => Type::Float32,
                        }
                    };
                }
                unit_operations!(Duration Length LogicalLength)
            }
        };
        Expression::BinaryExpression {
            lhs: Box::new(lhs.maybe_convert_to(expected_ty.clone(), None, &lhs_n, &mut ctx.diag)),
            rhs: Box::new(rhs.maybe_convert_to(expected_ty, None, &rhs_n, &mut ctx.diag)),
            op,
        }
    }

    fn from_unaryop_expression_node(
        node: syntax_nodes::UnaryOpExpression,
        ctx: &mut LookupCtx,
    ) -> Expression {
        let exp_n = node.Expression();
        let exp = Self::from_expression_node(exp_n.clone().into(), ctx);

        Expression::UnaryOp {
            sub: Box::new(exp),
            op: None
                .or(node.child_token(SyntaxKind::Plus).and(Some('+')))
                .or(node.child_token(SyntaxKind::Minus).and(Some('-')))
                .or(node.child_token(SyntaxKind::Bang).and(Some('!')))
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
            .maybe_convert_to(Type::Bool, None, &condition_n, &mut ctx.diag);
        let mut true_expr = Self::from_expression_node(true_expr_n.clone().into(), ctx);
        let mut false_expr = Self::from_expression_node(false_expr_n.clone().into(), ctx);
        let (true_ty, false_ty) = (true_expr.ty(), false_expr.ty());
        if true_ty != false_ty {
            if false_ty.can_convert(&true_ty) {
                false_expr =
                    false_expr.maybe_convert_to(true_ty, None, &false_expr_n, &mut ctx.diag);
            } else {
                true_expr = true_expr.maybe_convert_to(false_ty, None, &true_expr_n, &mut ctx.diag);
            }
        }
        Expression::Condition {
            condition: Box::new(condition),
            true_expr: Box::new(true_expr),
            false_expr: Box::new(false_expr),
        }
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
        let ty = Type::Object {
            fields: values.iter().map(|(k, v)| (k.clone(), v.ty())).collect(),
            name: None,
        };
        Expression::Object { ty, values }
    }

    fn from_array_node(node: syntax_nodes::Array, ctx: &mut LookupCtx) -> Expression {
        let mut values: Vec<Expression> =
            node.Expression().map(|e| Expression::from_expression_node(e, ctx)).collect();

        // FIXME: what's the type of an empty array ?
        // Also, be smarter about finding a common type
        let element_ty = values.first().map_or(Type::Invalid, |e| e.ty());

        for e in values.iter_mut() {
            *e = core::mem::replace(e, Expression::Invalid).maybe_convert_to(
                element_ty.clone(),
                None,
                &node,
                ctx.diag,
            );
        }

        Expression::Array { element_ty, values }
    }
}

fn continue_lookup_within_element(
    elem: &ElementRc,
    it: &mut impl Iterator<Item = crate::parser::SyntaxTokenWithSourceFile>,
    node: SyntaxNodeWithSourceFile,
    ctx: &mut LookupCtx,
) -> Expression {
    let second = if let Some(second) = it.next() {
        second
    } else if matches!(ctx.property_type, Type::ElementReference) {
        return Expression::ElementReference(Rc::downgrade(elem));
    } else {
        ctx.diag.push_error("Cannot take reference of an element".into(), &node);
        return Expression::Invalid;
    };
    let prop_name = crate::parser::normalize_identifier(second.text().as_str());

    let p = elem.borrow().lookup_property(&prop_name);
    if p.is_property_type() {
        let prop = Expression::PropertyReference(NamedReference {
            element: Rc::downgrade(elem),
            name: prop_name,
        });
        return maybe_lookup_object(prop, it, ctx);
    } else if matches!(p, Type::Signal{..}) {
        if let Some(x) = it.next() {
            ctx.diag.push_error("Cannot access fields of signal".into(), &x)
        }
        return Expression::SignalReference(NamedReference {
            element: Rc::downgrade(elem),
            name: prop_name.to_string(),
        });
    } else if matches!(p, Type::Function{..}) {
        let member = elem.borrow().base_type.lookup_member_function(&prop_name);
        return Expression::MemberFunction {
            base: Box::new(Expression::ElementReference(Rc::downgrade(elem))),
            base_node: node,
            member: Box::new(member),
        };
    } else {
        if let Some(minus_pos) = second.text().find('-') {
            // Attempt to recover if the user wanted to write "-"
            if elem.borrow().lookup_property(&second.text()[0..minus_pos]) != Type::Invalid {
                ctx.diag.push_error(format!("Cannot access property '{}'. Use space before the '-' if you meant a substraction.", second.text()), &second);
                return Expression::Invalid;
            }
        }
        ctx.diag.push_error(format!("Cannot access property '{}'", second.text()), &second);
        return Expression::Invalid;
    }
}

fn maybe_lookup_object(
    mut base: Expression,
    mut it: impl Iterator<Item = crate::parser::SyntaxTokenWithSourceFile>,
    ctx: &mut LookupCtx,
) -> Expression {
    fn error_or_try_minus(
        ctx: &mut LookupCtx,
        ident: crate::parser::SyntaxTokenWithSourceFile,
        lookup: impl Fn(&str) -> bool,
    ) -> Expression {
        if let Some(minus_pos) = ident.text().find('-') {
            if lookup(&ident.text()[0..minus_pos]) {
                ctx.diag.push_error(format!("Cannot access the field '{}'. Use space before the '-' if you meant a substraction.", ident.text()), &ident);
                return Expression::Invalid;
            }
        }
        ctx.diag.push_error(format!("Cannot access the field '{}'", ident.text()), &ident);
        Expression::Invalid
    }

    while let Some(next) = it.next() {
        let next_str = crate::parser::normalize_identifier(next.text().as_str());
        match base.ty() {
            Type::Object { fields, .. } => {
                if fields.get(next_str.as_str()).is_some() {
                    base = Expression::ObjectAccess {
                        base: Box::new(std::mem::replace(&mut base, Expression::Invalid)),
                        name: next_str,
                    }
                } else {
                    return error_or_try_minus(ctx, next, |x| fields.get(x).is_some());
                }
            }
            Type::Component(c) => {
                let prop_ty = c.root_element.borrow().lookup_property(next_str.as_str());
                if prop_ty != Type::Invalid {
                    base = Expression::ObjectAccess {
                        base: Box::new(std::mem::replace(&mut base, Expression::Invalid)),
                        name: next.to_string(),
                    }
                } else {
                    return error_or_try_minus(ctx, next, |x| {
                        c.root_element.borrow().lookup_property(x) != Type::Invalid
                    });
                }
            }
            _ => {
                ctx.diag.push_error("Cannot access fields of property".into(), &next);
                return Expression::Invalid;
            }
        }
    }
    base
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

fn parse_number_literal(s: String) -> Result<Expression, String> {
    let bytes = s.as_bytes();
    let mut end = 0;
    while end < bytes.len() && matches!(bytes[end], b'0'..=b'9' | b'.') {
        end += 1;
    }
    let val = s[..end].parse().map_err(|_| "Cannot parse number literal".to_owned())?;
    let unit = s[end..].parse().map_err(|_| "Invalid unit".to_owned())?;
    Ok(Expression::NumberLiteral(val, unit))
}

#[test]
fn test_parse_number_literal() {
    fn doit(s: &str) -> Result<(f64, Unit), String> {
        parse_number_literal(s.into()).map(|e| match e {
            Expression::NumberLiteral(a, b) => (a, b),
            _ => panic!(),
        })
    }

    assert_eq!(doit("10"), Ok((10., Unit::None)));
    assert_eq!(doit("10phx"), Ok((10., Unit::Phx)));
    assert_eq!(doit("10.0phx"), Ok((10., Unit::Phx)));
    assert_eq!(doit("10.0"), Ok((10., Unit::None)));
    assert_eq!(doit("1.1phx"), Ok((1.1, Unit::Phx)));
    assert_eq!(doit("10.10"), Ok((10.10, Unit::None)));
    assert_eq!(doit("10000000"), Ok((10000000., Unit::None)));
    assert_eq!(doit("10000001phx"), Ok((10000001., Unit::Phx)));

    let wrong_unit = Err("Invalid unit".to_owned());
    let cannot_parse = Err("Cannot parse number literal".to_owned());
    assert_eq!(doit("10000001 phx"), wrong_unit);
    assert_eq!(doit("12.10.12phx"), cannot_parse);
    assert_eq!(doit("12.12oo"), wrong_unit);
    assert_eq!(doit("12.12€"), wrong_unit);
}
