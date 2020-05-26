#![allow(dead_code)] // FIXME: remove when warnings are gone
use crate::diagnostics::Diagnostics;
use crate::object_tree::*;
use crate::parser::{Spanned, SyntaxKind, SyntaxNode, SyntaxNodeEx};
use crate::typeregister::{Type, TypeRegister};
use core::str::FromStr;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// Contains information which allow to lookup identifier in expressions
struct LookupCtx<'a> {
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

/// The Expression is hold by properties, so it should not hold any strong references to node from the object_tree
#[derive(Debug, Clone)]
pub enum Expression {
    /// Something went wrong (and an error will be reported)
    Invalid,
    /// We haven't done the lookup yet
    Uncompiled(SyntaxNode),
    /// A string literal. The .0 is the content of the string, without the quotes
    StringLiteral(String),
    /// Number
    NumberLiteral(f64),

    /// Reference to the signal <name> in the <element> within the <Component>
    ///
    /// Note: if we are to separate expression and statement, we probably do not need to have signal reference within expressions
    SignalReference { component: Weak<Component>, element: Weak<RefCell<Element>>, name: String },

    /// Reference to the signal <name> in the <element> within the <Component>
    ///
    /// Note: if we are to separate expression and statement, we probably do not need to have signal reference within expressions
    PropertyReference { component: Weak<Component>, element: Weak<RefCell<Element>>, name: String },
}

impl Expression {
    fn from_binding_expression_node(node: SyntaxNode, ctx: &mut LookupCtx) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::BindingExpression);

        node.child_node(SyntaxKind::Expression)
            .or_else(|| {
                node.child_node(SyntaxKind::CodeBlock)
                    .and_then(|c| c.child_node(SyntaxKind::Expression))
            })
            .map_or(Self::Invalid, |n| Self::from_expression_node(n, ctx))
    }

    fn from_expression_node(node: SyntaxNode, ctx: &mut LookupCtx) -> Self {
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
                let path = std::path::Path::new(&s);

                if path.is_absolute() {
                    return Expression::StringLiteral(s);
                }
                let path = ctx.diag.path(node.span()).parent().unwrap().join(path);
                if path.is_absolute() {
                    return Expression::StringLiteral(path.to_string_lossy().to_string());
                }
                Expression::StringLiteral(
                    std::env::current_dir().unwrap().join(path).to_string_lossy().to_string(),
                )
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
            return Self::PropertyReference {
                component: Rc::downgrade(&ctx.component),
                element: Rc::downgrade(&ctx.component.root_element),
                name: s.to_string(),
            };
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
                    )
                }
                return Self::PropertyReference {
                    component: Rc::downgrade(&ctx.component),
                    element: Rc::downgrade(&elem),
                    name: prop_name.text().to_string(),
                };
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

        if matches!(ctx.property_type, Type::Signal) {
            if let Some(x) = it.next() {
                ctx.diag.push_error(
                    "Cannot access fields of signal".into(),
                    x.into_token().unwrap().span(),
                )
            }
            return Self::SignalReference {
                component: Rc::downgrade(&ctx.component),
                element: Rc::downgrade(&ctx.component.root_element),
                name: s.to_string(),
            };
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
                // FIXME: there should be a ColorLiteral
                return Self::NumberLiteral(value as f64);
            }
        }

        ctx.diag.push_error(format!("Unknown unqualified identifier '{}'", s), node.span());

        Self::Invalid
    }
}

fn unescape_string(string: &str) -> Option<String> {
    if !string.starts_with('"') || !string.ends_with('"') {
        return None;
    }
    let string = &string[1..(string.len() - 1)];
    // TODO: remove slashes
    return Some(string.into());
}

pub fn resolve_expressions(doc: &Document, diag: &mut Diagnostics, tr: &mut TypeRegister) {
    fn resolve_expressions_in_element_recursively(
        elem: &Rc<RefCell<Element>>,
        component: &Rc<Component>,
        diag: &mut Diagnostics,
        tr: &mut TypeRegister,
    ) {
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

        for child in &elem.borrow().children {
            resolve_expressions_in_element_recursively(child, component, diag, tr);
        }
    }
    for x in &doc.inner_components {
        resolve_expressions_in_element_recursively(&x.root_element, x, diag, tr)
    }
}
