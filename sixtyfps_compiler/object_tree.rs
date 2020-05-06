use crate::diagnostics::Diagnostics;
use crate::parser::{SyntaxKind, SyntaxNode, SyntaxNodeEx};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Default, Debug)]
pub struct Document {
    //     node: SyntaxNode,
    pub root_component: Rc<Component>,
}

impl Document {
    pub fn from_node(node: SyntaxNode, diag: &mut Diagnostics) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Document);
        Document {
            root_component: Rc::new(
                node.child_node(SyntaxKind::Component)
                    .map_or_else(Default::default, |n| Component::from_node(n, diag)),
            ),
        }
    }
}

#[derive(Default, Debug)]
pub struct Component {
    //     node: SyntaxNode,
    pub id: String,
    pub root_element: Rc<Element>,
}

impl Component {
    pub fn from_node(node: SyntaxNode, diag: &mut Diagnostics) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Component);
        Component {
            id: node.child_text(SyntaxKind::Identifier).unwrap_or_default(),
            root_element: Rc::new(
                node.child_node(SyntaxKind::Element)
                    .map_or_else(Default::default, |n| Element::from_node(n, "root".into(), diag)),
            ),
        }
    }
}

#[derive(Default, Debug)]
pub struct Element {
    //     node: SyntaxNode,
    pub id: String,
    pub base: String,
    pub bindings: HashMap<String, CodeStatement>,
    pub children: Vec<Rc<Element>>,
}

impl Element {
    pub fn from_node(node: SyntaxNode, id: String, diag: &mut Diagnostics) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Element);
        let mut r = Element {
            id,
            base: node.child_text(SyntaxKind::Identifier).unwrap_or_default(),
            ..Default::default()
        };
        for b in node.children().filter(|n| n.kind() == SyntaxKind::Binding) {
            let name_token = match b.child_token(SyntaxKind::Identifier) {
                Some(x) => x,
                None => continue,
            };
            let name = name_token.text().to_string();
            if let Some(csn) = b.child_node(SyntaxKind::CodeStatement) {
                if r.bindings.insert(name, CodeStatement::from_node(csn, diag)).is_some() {
                    diag.push_error(
                        "Duplicated property".into(),
                        name_token.text_range().start().into(),
                    );
                }
            }
        }
        for se in node.children().filter(|n| n.kind() == SyntaxKind::SubElement) {
            let id = se.child_text(SyntaxKind::Identifier).unwrap_or_default();
            if let Some(element_node) = se.child_node(SyntaxKind::Element) {
                r.children.push(Rc::new(Element::from_node(element_node, id, diag)));
            } else {
                assert!(diag.has_error());
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
        // FIXME
        CodeStatement {
            value: node
                .child_node(SyntaxKind::Expression)
                .and_then(|x| x.child_text(SyntaxKind::Identifier))
                .unwrap_or_default(),
        }
    }
}
