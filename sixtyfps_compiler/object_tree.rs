/*!
 This module contains the intermediate representation of the code in the form of an object tree
*/

use crate::diagnostics::Diagnostics;
use crate::expression_tree::Expression;
use crate::parser::{Spanned, SyntaxKind, SyntaxNode, SyntaxNodeEx};
use crate::typeregister::{Type, TypeRegister};
use std::cell::RefCell;
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
    pub root_element: Rc<RefCell<Element>>,
}

impl Component {
    pub fn from_node(node: SyntaxNode, diag: &mut Diagnostics, tr: &TypeRegister) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Component);
        Component {
            id: node.child_text(SyntaxKind::Identifier).unwrap_or_default(),
            root_element: Rc::new(RefCell::new(
                node.child_node(SyntaxKind::Element).map_or_else(Default::default, |n| {
                    Element::from_node(n, "root".into(), diag, tr)
                }),
            )),
        }
    }
}

/// An Element is an instentation of a Component
#[derive(Default, Debug)]
pub struct Element {
    /* node: SyntaxNode, */
    /// The id as named in the original .60 file
    pub id: String,
    pub lowered_id: String,
    pub base: QualifiedTypeName,
    pub base_type: crate::typeregister::Type,
    /// Currently contains also the signals. FIXME: should that be changed?
    pub bindings: HashMap<String, Expression>,
    pub children: Vec<Rc<RefCell<Element>>>,

    /// This should probably be in the Component instead
    pub signals_declaration: Vec<String>,
    pub property_declarations: HashMap<String, Type>,
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
            diag.push_error(format!("Unknown type {}", r.base), node.span());
            return r;
        }

        for prop_decl in node.children().filter(|n| n.kind() == SyntaxKind::PropertyDeclaration) {
            let qualified_type_node = prop_decl
                .children()
                .filter(|n| n.kind() == SyntaxKind::QualifiedTypeName)
                .nth(0)
                .unwrap();
            let type_span = qualified_type_node.span();
            let qualified_type = QualifiedTypeName::from_node(qualified_type_node);

            let prop_type = tr.lookup_qualified(&qualified_type.members);

            match prop_type {
                Type::Invalid => {
                    diag.push_error(
                        format!("unknown property type '{}'", qualified_type.to_string()),
                        type_span,
                    );
                }
                _ => (),
            };

            let prop_name_token = match prop_decl
                .children_with_tokens()
                .filter(|n| n.kind() == SyntaxKind::Identifier)
                .last()
            {
                Some(x) => x.into_token().unwrap(),
                None => continue,
            };
            let prop_name = prop_name_token.text().to_string();

            if r.property_declarations.insert(prop_name.clone(), prop_type).is_some() {
                diag.push_error(
                    "Duplicated property declaration".into(),
                    crate::diagnostics::Span::new(prop_name_token.text_range().start().into()),
                )
            }

            if let Some(csn) = prop_decl.child_node(SyntaxKind::BindingExpression) {
                if r.bindings.insert(prop_name, Expression::Uncompiled(csn)).is_some() {
                    diag.push_error(
                        "Duplicated property binding".into(),
                        crate::diagnostics::Span::new(prop_name_token.text_range().start().into()),
                    );
                }
            }
        }

        for b in node.children().filter(|n| n.kind() == SyntaxKind::Binding) {
            let name_token = match b.child_token(SyntaxKind::Identifier) {
                Some(x) => x,
                None => continue,
            };
            let name = name_token.text().to_string();
            let prop_type = r.lookup_property(&name);
            if !prop_type.is_property_type() {
                diag.push_error(
                    match prop_type {
                        Type::Invalid => format!("Unknown property {} in {}", name, r.base),
                        Type::Signal => format!("'{}' is a signal. use `=>` to connect", name),
                        _ => format!("Cannot assing to {} in {}", name, r.base),
                    },
                    crate::diagnostics::Span::new(name_token.text_range().start().into()),
                );
            }
            if let Some(csn) = b.child_node(SyntaxKind::BindingExpression) {
                if r.bindings.insert(name, Expression::Uncompiled(csn)).is_some() {
                    diag.push_error(
                        "Duplicated property binding".into(),
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
                if r.bindings.insert(name, Expression::Uncompiled(csn)).is_some() {
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
                    r.children.push(Rc::new(RefCell::new(Element::from_node(
                        element_node,
                        id,
                        diag,
                        tr,
                    ))));
                } else {
                    assert!(diag.has_error());
                }
            } else if se.kind() == SyntaxKind::RepeatedElement {
                diag.push_error("TODO: for not implemented".to_owned(), se.span())
            }
        }
        r
    }

    pub fn lookup_property(&self, name: &str) -> Type {
        self.property_declarations
            .get(name)
            .cloned()
            .unwrap_or_else(|| self.base_type.lookup_property(name))
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
