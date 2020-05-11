/*!
 This module contains the intermediate representation of the code in the form of an object tree
*/

use crate::diagnostics::Diagnostics;
use crate::parser::{SyntaxKind, SyntaxNode, SyntaxNodeEx};
use crate::typeregister::TypeRegister;
use std::collections::HashMap;
use std::rc::Rc;
/// The full document (a complete file)
#[derive(Default, Debug)]
pub struct Document {
    //     node: SyntaxNode,
    pub root_component: Rc<Component>,
}

impl Document {
    pub fn from_node(node: SyntaxNode, diag: &mut Diagnostics, tr: &TypeRegister) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Document);
        Document {
            root_component: Rc::new(
                node.child_node(SyntaxKind::Component)
                    .map_or_else(Default::default, |n| Component::from_node(n, diag, tr)),
            ),
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
    pub base: String,
    /// FIXME, should not be Builtin, but a generic Component as well
    pub base_type: Rc<crate::typeregister::BuiltinElement>,
    pub bindings: HashMap<String, CodeStatement>,
    pub children: Vec<Rc<Element>>,
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
            base: node.child_text(SyntaxKind::Identifier).unwrap_or_default(),
            ..Default::default()
        };
        r.base_type = if let Some(ty) = tr.lookup(&r.base) {
            ty
        } else {
            diag.push_error(format!("Unkown type {}", r.base), node.text_range().start().into());
            return r;
        };
        for b in node.children().filter(|n| n.kind() == SyntaxKind::Binding) {
            let name_token = match b.child_token(SyntaxKind::Identifier) {
                Some(x) => x,
                None => continue,
            };
            let name = name_token.text().to_string();
            if !r.base_type.properties.contains_key(&name) {
                diag.push_error(
                    format!("Unkown property {} in {}", name, r.base),
                    name_token.text_range().start().into(),
                );
            }
            if let Some(csn) = b.child_node(SyntaxKind::CodeStatement) {
                if r.bindings.insert(name, CodeStatement::from_node(csn, diag)).is_some() {
                    diag.push_error(
                        "Duplicated property".into(),
                        name_token.text_range().start().into(),
                    );
                }
            }
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
                diag.push_error(
                    "TODO: for not implemented".to_owned(),
                    se.text_range().start().into(),
                )
            }
        }
        r
    }
}

#[derive(Default, Debug)]
pub struct CodeStatement {
    //     node: SyntaxNode,
    pub value: String,
}

impl CodeStatement {
    pub fn from_node(node: SyntaxNode, _diag: &mut Diagnostics) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::CodeStatement);

        fn from_expression(x: SyntaxNode) -> String {
            x.child_node(SyntaxKind::Expression)
                .map(from_expression)
                .or_else(|| x.child_text(SyntaxKind::Identifier))
                .or_else(|| x.child_text(SyntaxKind::StringLiteral))
                .or_else(|| x.child_text(SyntaxKind::NumberLiteral))
                .unwrap_or_default()
        }

        let value = node
            .child_node(SyntaxKind::Expression)
            .or_else(|| {
                node.child_node(SyntaxKind::CodeBlock)
                    .and_then(|c| c.child_node(SyntaxKind::Expression))
            })
            .map_or(Default::default(), from_expression);

        // FIXME: that's not the place to do this
        let value = match &*value {
            "blue" => "0xff0000ff",
            "red" => "0xffff0000",
            "green" => "0xff00ff00",
            "yellow" => "0xffffff00",
            "black" => "0xff000000",
            "white" => "0xffffffff",
            _ => &value,
        }
        .into();

        // FIXME
        CodeStatement { value }
    }
}
