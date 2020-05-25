/*!
 This module contains the intermediate representation of the code in the form of an object tree
*/

use crate::diagnostics::Diagnostics;
use crate::parser::{SyntaxKind, SyntaxNode, SyntaxNodeEx};
use crate::typeregister::{Type, TypeRegister};
use core::str::FromStr;
use std::collections::HashMap;
use std::rc::Rc;
/// The full document (a complete file)
#[derive(Default, Debug)]
pub struct Document {
    //     node: SyntaxNode,
    pub inner_components: Vec<Rc<Component>>,
    pub root_component: Rc<Component>,
}

impl Document {
    pub fn from_node(node: SyntaxNode, diag: &mut Diagnostics, tr: &mut TypeRegister) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Document);

        let inner_components = node
            .children()
            .filter(|n| n.kind() == SyntaxKind::Component)
            .map(|n| {
                let compo = Rc::new(Component::from_node(n, diag, tr));
                tr.add(compo.clone());
                compo
            })
            .collect::<Vec<_>>();

        Document {
            // FIXME: one should use the `component` hint instead of always returning the last
            root_component: inner_components.last().cloned().unwrap_or_default(),

            inner_components,
        }
    }
}

/// A component is a type in the language which can be instantiated
#[derive(Default, Debug)]
pub struct Component {
    //     node: SyntaxNode,
    pub id: String,
    pub root_element: Rc<Element>,
}

impl Component {
    pub fn from_node(node: SyntaxNode, diag: &mut Diagnostics, tr: &TypeRegister) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Component);
        Component {
            id: node.child_text(SyntaxKind::Identifier).unwrap_or_default(),
            root_element: Rc::new(
                node.child_node(SyntaxKind::Element).map_or_else(Default::default, |n| {
                    Element::from_node(n, "root".into(), diag, tr)
                }),
            ),
        }
    }
}

/// An Element is an instentation of a Component
#[derive(Default, Debug)]
pub struct Element {
    //     node: SyntaxNode,
    pub id: String,
    pub base: QualifiedTypeName,
    pub base_type: crate::typeregister::Type,
    /// Currently contains also the signals. FIXME: should that be changed?
    pub bindings: HashMap<String, Expression>,
    pub children: Vec<Rc<Element>>,

    /// This should probably be in the Component instead
    pub signals_declaration: Vec<String>,
}

impl Element {
    pub fn from_node(
        node: SyntaxNode,
        id: String,
        diag: &mut Diagnostics,
        tr: &TypeRegister,
    ) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Element);
        let mut r = Element {
            id,
            base: QualifiedTypeName::from_node(
                node.children()
                    .filter(|n| n.kind() == SyntaxKind::QualifiedTypeName)
                    .nth(0)
                    .unwrap(),
            ),
            ..Default::default()
        };
        r.base_type = tr.lookup(&r.base.to_string());
        if !r.base_type.is_object_type() {
            diag.push_error(format!("Unkown type {}", r.base), node.span());
            return r;
        }
        for b in node.children().filter(|n| n.kind() == SyntaxKind::Binding) {
            let name_token = match b.child_token(SyntaxKind::Identifier) {
                Some(x) => x,
                None => continue,
            };
            let name = name_token.text().to_string();
            let prop_type = r.base_type.lookup_property(&name);
            if !prop_type.is_property_type() {
                diag.push_error(
                    match prop_type {
                        Type::Invalid => format!("Unkown property {} in {}", name, r.base),
                        Type::Signal => format!("'{}' is a signal. use `=>` to connect", name),
                        _ => format!("Cannot assing to {} in {}", name, r.base),
                    },
                    crate::diagnostics::Span::new(name_token.text_range().start().into()),
                );
            }
            if let Some(csn) = b.child_node(SyntaxKind::CodeStatement) {
                if r.bindings
                    .insert(name, Expression::from_code_statement_node(csn, diag))
                    .is_some()
                {
                    diag.push_error(
                        "Duplicated property".into(),
                        crate::diagnostics::Span::new(name_token.text_range().start().into()),
                    );
                }
            }
        }
        for con_node in node.children().filter(|n| n.kind() == SyntaxKind::SignalConnection) {
            let name_token = match con_node.child_token(SyntaxKind::Identifier) {
                Some(x) => x,
                None => continue,
            };
            let name = name_token.text().to_string();
            let prop_type = r.base_type.lookup_property(&name);
            if !matches!(prop_type, Type::Signal) {
                diag.push_error(
                    format!("'{}' is not a signal in {}", name, r.base),
                    crate::diagnostics::Span::new(name_token.text_range().start().into()),
                );
            }
            if let Some(csn) = con_node.child_node(SyntaxKind::CodeBlock) {
                let ex = csn
                    .child_node(SyntaxKind::Expression)
                    .map(|en| Expression::from_expression_node(en, diag))
                    .unwrap_or(Expression::Invalid);
                if r.bindings.insert(name, ex).is_some() {
                    diag.push_error(
                        "Duplicated signal".into(),
                        crate::diagnostics::Span::new(name_token.text_range().start().into()),
                    );
                }
            }
        }

        for sig_decl in node.children().filter(|n| n.kind() == SyntaxKind::SignalDeclaration) {
            // We need to go reverse to skip the "signal" token
            let name_token = match sig_decl
                .children_with_tokens()
                .filter(|n| n.kind() == SyntaxKind::Identifier)
                .last()
            {
                Some(x) => x.into_token().unwrap(),
                None => continue,
            };
            let name = name_token.text().to_string();
            r.signals_declaration.push(name);
        }

        for se in node.children() {
            if se.kind() == SyntaxKind::SubElement {
                let id = se.child_text(SyntaxKind::Identifier).unwrap_or_default();
                if let Some(element_node) = se.child_node(SyntaxKind::Element) {
                    r.children.push(Rc::new(Element::from_node(element_node, id, diag, tr)));
                } else {
                    assert!(diag.has_error());
                }
            } else if se.kind() == SyntaxKind::RepeatedElement {
                diag.push_error("TODO: for not implemented".to_owned(), se.span())
            }
        }
        r
    }
}

#[derive(Default, Debug, Clone)]
pub struct QualifiedTypeName {
    members: Vec<String>,
}

impl QualifiedTypeName {
    pub fn from_node(node: SyntaxNode) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::QualifiedTypeName);
        let members = node
            .children_with_tokens()
            .filter(|n| n.kind() == SyntaxKind::Identifier)
            .filter_map(|x| x.as_token().map(|x| x.text().to_string()))
            .collect();
        Self { members }
    }
}

impl std::fmt::Display for QualifiedTypeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.members.join("."))
    }
}

#[derive(Debug, Clone)]
pub enum Expression {
    Invalid,
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
