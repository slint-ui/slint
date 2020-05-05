use crate::diagnostics::Diagnostics;
use crate::parser::{SyntaxKind, SyntaxNode, SyntaxNodeEx};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Default, Debug)]
pub struct Document {
    //     node: SyntaxNode,
    root_component: Rc<Component>,
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
    id: String,
    root_element: Rc<Element>,
}

impl Component {
    pub fn from_node(node: SyntaxNode, diag: &mut Diagnostics) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Component);
        Component {
            id: node.child_text(SyntaxKind::Identifier).unwrap_or_default(),
            root_element: Rc::new(
                node.child_node(SyntaxKind::Element)
                    .map_or_else(Default::default, |n| Element::from_node(n, diag)),
            ),
        }
    }
}

#[derive(Default, Debug)]
pub struct Element {
    //     node: SyntaxNode,
    base: String,
    bindings: HashMap<String, CodeStatement>,
}

impl Element {
    pub fn from_node(node: SyntaxNode, diag: &mut Diagnostics) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Element);
        let mut r = Element {
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
        r
    }
}

#[derive(Default, Debug)]
pub struct CodeStatement {
    //     node: SyntaxNode,
    value: String,
}

impl CodeStatement {
    pub fn from_node(node: SyntaxNode, _diag: &mut Diagnostics) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::CodeStatement);
        // FIXME
        CodeStatement { value: node.child_text(SyntaxKind::Identifier).unwrap_or_default() }
    }
}
