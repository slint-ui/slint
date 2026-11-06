// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::object_tree::{ElementLocation, ElementProperty, ElementRc};
use i_slint_compiler::parser::{SyntaxKind, SyntaxNode, TextSize, syntax_nodes};

/// Marks nodes that the language server and preview ignore during code analysis.
pub const NODE_IGNORE_COMMENT: &str = "@lsp:ignore-node";

/// Returns whether an element is marked to be ignored during code analysis.
pub fn is_element_node_ignored(node: &syntax_nodes::Element) -> bool {
    node.children_with_tokens().any(|node_or_token| {
        node_or_token
            .as_token()
            .map(|token| {
                token.kind() == SyntaxKind::Comment && token.text().contains(NODE_IGNORE_COMMENT)
            })
            .unwrap_or(false)
    })
}

pub fn extract_element(node: SyntaxNode) -> Option<syntax_nodes::Element> {
    match node.kind() {
        SyntaxKind::Element => Some(node.into()),
        SyntaxKind::SubElement => extract_element(node.child_node(SyntaxKind::Element)?),
        SyntaxKind::ConditionalElement | SyntaxKind::RepeatedElement => {
            extract_element(node.child_node(SyntaxKind::SubElement)?)
        }
        _ => None,
    }
}

fn find_element_with_decoration(element: &syntax_nodes::Element) -> SyntaxNode {
    let this_node: SyntaxNode = element.clone().into();
    element
        .parent()
        .and_then(|parent| match parent.kind() {
            SyntaxKind::SubElement => parent.parent().map(|grandparent| {
                if grandparent.kind() == SyntaxKind::ConditionalElement
                    || grandparent.kind() == SyntaxKind::RepeatedElement
                {
                    grandparent
                } else {
                    parent
                }
            }),
            _ => Some(this_node.clone()),
        })
        .unwrap_or(this_node)
}

fn find_parent_component(node: &SyntaxNode) -> Option<SyntaxNode> {
    let mut current = Some(node.clone());
    while let Some(parent) = current {
        if matches!(parent.kind(), SyntaxKind::Component) {
            return Some(parent);
        }
        current = parent.parent();
    }
    None
}

#[derive(Clone)]
pub struct ElementRcNode {
    pub element: ElementRc,
    pub debug_index: usize,
}

impl std::cmp::PartialEq for ElementRcNode {
    fn eq(&self, other: &Self) -> bool {
        self.location() == other.location() && self.debug_index == other.debug_index
    }
}

impl std::fmt::Debug for ElementRcNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ElementLocation { path, offset } = self.location();
        write!(formatter, "ElementNode {{ {path:?}:{offset:?} }}")
    }
}

impl ElementRcNode {
    pub fn new(element: ElementRc, debug_index: usize) -> Option<Self> {
        let _ = element.borrow().debug.get(debug_index)?;

        Some(Self { element, debug_index })
    }

    pub fn next_element_rc_node(&self) -> Option<Self> {
        Self::new(self.element.clone(), self.debug_index + 1)
    }

    pub fn find_in_or_below(element: ElementRc, location: &ElementLocation) -> Option<Self> {
        let debug_index = element.borrow().debug.iter().position(|debug_info| {
            debug_info.node.text_range().start() == location.offset
                && *debug_info.node.source_file.path() == location.path
        });
        if let Some(debug_index) = debug_index {
            Some(Self { element, debug_index })
        } else {
            for child in &element.borrow().children {
                let result = Self::find_in_or_below(child.clone(), location);
                if result.is_some() {
                    return result;
                }
            }
            None
        }
    }

    pub fn with_element_debug<Result>(
        &self,
        function: impl FnOnce(&i_slint_compiler::object_tree::ElementDebugInfo) -> Result,
    ) -> Result {
        let element = self.element.borrow();
        let debug_info = element.debug.get(self.debug_index).unwrap();
        function(debug_info)
    }

    pub fn with_element_node<Result>(
        &self,
        function: impl FnOnce(&i_slint_compiler::parser::syntax_nodes::Element) -> Result,
    ) -> Result {
        let element = self.element.borrow();
        function(&element.debug.get(self.debug_index).unwrap().node)
    }

    pub fn with_decorated_node<Result>(
        &self,
        function: impl FnOnce(SyntaxNode) -> Result,
    ) -> Result {
        let element = self.element.borrow();
        function(find_element_with_decoration(&element.debug.get(self.debug_index).unwrap().node))
    }

    pub fn location(&self) -> ElementLocation {
        self.with_element_node(ElementLocation::of)
    }

    /// Returns the property `name` of this element, such as the one a debug hook observes.
    pub fn property(&self, name: &str) -> ElementProperty {
        self.location().property(name)
    }

    pub fn as_element(&self) -> &ElementRc {
        &self.element
    }

    pub fn parent(&self) -> Option<ElementRcNode> {
        let mut ancestor = self.with_element_node(|node| node.parent());

        while let Some(parent) = ancestor {
            if parent.kind() != SyntaxKind::Element {
                ancestor = parent.parent();
                continue;
            }

            let parent_location = ElementLocation::of(&parent.clone().into());

            ancestor = parent.parent();

            let component = self.element.borrow().enclosing_component.upgrade().unwrap();
            let current_root = component.root_element.clone();
            let root_element = if std::rc::Rc::ptr_eq(&current_root, &self.element) {
                component.parent_element().map_or(current_root, |parent| {
                    parent.borrow().enclosing_component.upgrade().unwrap().root_element.clone()
                })
            } else {
                current_root
            };

            let result = Self::find_in_or_below(root_element, &parent_location);

            if result.is_some() {
                return result;
            }
        }

        None
    }

    pub fn children(&self) -> Vec<ElementRcNode> {
        self.with_element_node(|node| {
            let mut children = Vec::new();
            for child in node.children() {
                if let Some(element) = extract_element(child.clone()) {
                    let Some(child_node) = ElementRcNode::find_in_or_below(
                        self.as_element().clone(),
                        &ElementLocation::of(&element),
                    ) else {
                        continue;
                    };
                    children.push(child_node);
                }
            }

            children
        })
    }

    pub fn component_type(&self) -> String {
        self.with_element_node(|node| {
            node.QualifiedName().map(|name| name.text().to_string()).unwrap_or_default()
        })
    }

    pub fn is_same_component_as(&self, other: &Self) -> bool {
        let Some(self_component) = self.with_element_node(|node| find_parent_component(node))
        else {
            return false;
        };
        let Some(other_component) = other.with_element_node(|node| find_parent_component(node))
        else {
            return false;
        };

        std::sync::Arc::ptr_eq(&self_component.source_file, &other_component.source_file)
            && self_component.text_range() == other_component.text_range()
    }

    pub fn contains_offset(&self, offset: TextSize) -> bool {
        self.with_element_node(|node| {
            node.parent().is_some_and(|parent| parent.text_range().contains(offset))
        })
    }
}
