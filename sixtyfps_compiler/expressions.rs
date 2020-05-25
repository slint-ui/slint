use crate::diagnostics::Diagnostics;
use crate::object_tree::*;
use crate::parser::{SyntaxKind, SyntaxNode, SyntaxNodeEx};
use crate::typeregister::{Type, TypeRegister};
use core::str::FromStr;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum Expression {
    /// Something went wrong (and an error will be reported)
    Invalid,
    /// We haven't done the lookup yet
    Uncompiled(SyntaxNode),
    /// A simple identifier, for example `something`, the .0 is then "something"
    Identifier(String),
    /// A string literal. The .0 is the content of the string, without the quotes
    StringLiteral(String),
    /// Number
    NumberLiteral(f64),
}

impl Expression {
    pub fn from_code_statement_node(node: SyntaxNode, diag: &mut Diagnostics) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::CodeStatement);

        node.child_node(SyntaxKind::Expression)
            .or_else(|| {
                node.child_node(SyntaxKind::CodeBlock)
                    .and_then(|c| c.child_node(SyntaxKind::Expression))
            })
            .map_or(Self::Invalid, |n| Self::from_expression_node(n, diag))
    }

    pub fn from_expression_node(node: SyntaxNode, diag: &mut Diagnostics) -> Self {
        node.child_node(SyntaxKind::Expression)
            .map(|n| Self::from_expression_node(n, diag))
            .or_else(|| {
                node.child_node(SyntaxKind::BangExpression)
                    .map(|n| Self::from_bang_expresion_node(n, diag))
            })
            .or_else(|| node.child_text(SyntaxKind::Identifier).map(|s| Self::Identifier(s)))
            .or_else(|| {
                node.child_text(SyntaxKind::StringLiteral).map(|s| {
                    unescape_string(&s).map(Self::StringLiteral).unwrap_or_else(|| {
                        diag.push_error("Cannot parse string literal".into(), node.span());
                        Self::Invalid
                    })
                })
            })
            .or_else(|| {
                node.child_text(SyntaxKind::NumberLiteral).map(|s| {
                    f64::from_str(&s).ok().map(Self::NumberLiteral).unwrap_or_else(|| {
                        diag.push_error("Cannot parse number literal".into(), node.span());
                        Self::Invalid
                    })
                })
            })
            .unwrap_or(Self::Invalid)
    }

    fn from_bang_expresion_node(node: SyntaxNode, diag: &mut Diagnostics) -> Self {
        match node.child_text(SyntaxKind::Identifier).as_ref().map(|x| x.as_str()) {
            None => {
                debug_assert!(false, "the parser should not allow that");
                diag.push_error("Missing bang keyword".into(), node.span());
                return Self::Invalid;
            }
            Some("img") => {
                // FIXME: we probably need a better syntax and make this at another level.
                let s = match node
                    .child_node(SyntaxKind::Expression)
                    .map_or(Self::Invalid, |n| Self::from_expression_node(n, diag))
                {
                    Expression::StringLiteral(p) => p,
                    _ => {
                        diag.push_error(
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
                let path = diag.path(node.span()).parent().unwrap().join(path);
                if path.is_absolute() {
                    return Expression::StringLiteral(path.to_string_lossy().to_string());
                }
                Expression::StringLiteral(
                    std::env::current_dir().unwrap().join(path).to_string_lossy().to_string(),
                )
            }
            Some(x) => {
                diag.push_error(format!("Unkown bang keyword `{}`", x), node.span());
                return Self::Invalid;
            }
        }
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
        diag: &mut Diagnostics,
        tr: &mut TypeRegister,
    ) {
        let base = elem.borrow().base_type.clone();
        for (prop, expr) in &mut elem.borrow_mut().bindings {
            if let Expression::Uncompiled(node) = expr {
                let new_expr = if matches!(base.lookup_property(&*prop), Type::Signal) {
                    //FIXME: proper signal suport (node is a codeblock)
                    node.child_node(SyntaxKind::Expression)
                        .map(|en| Expression::from_expression_node(en, diag))
                        .unwrap_or(Expression::Invalid)
                } else {
                    Expression::from_code_statement_node(node.clone(), diag)
                };
                *expr = new_expr;
            }
        }

        for child in &elem.borrow().children {
            resolve_expressions_in_element_recursively(child, diag, tr);
        }
    }
    for x in &doc.inner_components {
        resolve_expressions_in_element_recursively(&x.root_element, diag, tr)
    }
}
