// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};

use i_slint_compiler::{
    object_tree::ElementRc,
    parser::{SyntaxKind, SyntaxNode, TextSize, syntax_nodes},
};

use crate::document_cache::DocumentCache;

#[derive(Clone)]
pub struct ElementRcNode {
    pub element: ElementRc,
    pub debug_index: usize,
}

impl std::cmp::PartialEq for ElementRcNode {
    fn eq(&self, other: &Self) -> bool {
        self.path_and_offset() == other.path_and_offset() && self.debug_index == other.debug_index
    }
}

impl std::fmt::Debug for ElementRcNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (path, offset) = self.path_and_offset();
        write!(f, "ElementNode {{ {path:?}:{offset:?} }}")
    }
}

impl ElementRcNode {
    pub fn new(element: ElementRc, debug_index: usize) -> Option<Self> {
        let _ = element.borrow().debug.get(debug_index)?;

        Some(Self { element, debug_index })
    }

    pub fn in_document_cache(&self, document_cache: &DocumentCache) -> Option<Self> {
        self.with_element_node(|en| {
            let element_start = en.text_range().start();
            let path = en.source_file.path();

            let doc = document_cache.get_document_by_path(path)?;
            let component = doc.inner_components.iter().find(|c| {
                let Some(c_node) = &c.node else {
                    return false;
                };
                c_node.text_range().contains(element_start)
            })?;
            ElementRcNode::find_in_or_below(
                component.root_element.clone(),
                path,
                u32::from(element_start),
            )
        })
    }

    /// Some nodes get merged into the same ElementRc with no real connections between them...
    pub fn next_element_rc_node(&self) -> Option<Self> {
        Self::new(self.element.clone(), self.debug_index + 1)
    }

    pub fn find_in(element: ElementRc, path: &Path, offset: u32) -> Option<Self> {
        let debug_index = element.borrow().debug.iter().position(|d| {
            u32::from(d.node.text_range().start()) == offset && d.node.source_file.path() == path
        })?;

        Some(Self { element, debug_index })
    }

    pub fn find_in_or_below(element: ElementRc, path: &Path, offset: u32) -> Option<Self> {
        let debug_index = element.borrow().debug.iter().position(|d| {
            u32::from(d.node.text_range().start()) == offset && d.node.source_file.path() == path
        });
        if let Some(debug_index) = debug_index {
            Some(Self { element, debug_index })
        } else {
            for c in &element.borrow().children {
                let result = Self::find_in_or_below(c.clone(), path, offset);
                if result.is_some() {
                    return result;
                }
            }
            None
        }
    }

    /// Run with all the debug information on the node
    pub fn with_element_debug<R>(
        &self,
        func: impl FnOnce(&i_slint_compiler::object_tree::ElementDebugInfo) -> R,
    ) -> R {
        let elem = self.element.borrow();
        let d = elem.debug.get(self.debug_index).unwrap();
        func(d)
    }

    /// Run with the `Element` node
    pub fn with_element_node<R>(
        &self,
        func: impl FnOnce(&i_slint_compiler::parser::syntax_nodes::Element) -> R,
    ) -> R {
        let elem = self.element.borrow();
        func(&elem.debug.get(self.debug_index).unwrap().node)
    }

    /// Run with the SyntaxNode incl. any id, condition, etc.
    pub fn with_decorated_node<R>(&self, func: impl FnOnce(SyntaxNode) -> R) -> R {
        let elem = self.element.borrow();
        func(find_element_with_decoration(&elem.debug.get(self.debug_index).unwrap().node))
    }

    pub fn path_and_offset(&self) -> (PathBuf, TextSize) {
        self.with_element_node(|n| (n.source_file.path().to_owned(), n.text_range().start()))
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

            let (parent_path, parent_offset) =
                (parent.source_file.path().to_owned(), u32::from(parent.text_range().start()));

            ancestor = parent.parent();

            let component = self.element.borrow().enclosing_component.upgrade().unwrap();
            let current_root = component.root_element.clone();
            let root_element = if std::rc::Rc::ptr_eq(&current_root, &self.element) {
                component.parent_element.upgrade().map_or(current_root, |parent| {
                    parent.borrow().enclosing_component.upgrade().unwrap().root_element.clone()
                })
            } else {
                current_root
            };

            let result = Self::find_in_or_below(root_element, &parent_path, parent_offset);

            if result.is_some() {
                return result;
            }
        }

        None
    }

    pub fn children(&self) -> Vec<ElementRcNode> {
        self.with_element_node(|node| {
            let mut children = Vec::new();
            for c in node.children() {
                if let Some(element) = extract_element(c.clone()) {
                    let e_path = element.source_file.path().to_path_buf();
                    let e_offset = u32::from(element.text_range().start());

                    let Some(child_node) = ElementRcNode::find_in_or_below(
                        self.as_element().clone(),
                        &e_path,
                        e_offset,
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
            node.QualifiedName().map(|qn| qn.text().to_string()).unwrap_or_default()
        })
    }

    pub fn is_same_component_as(&self, other: &Self) -> bool {
        let Some(s) = self.with_element_node(|n| find_parent_component(n)) else {
            return false;
        };
        let Some(o) = other.with_element_node(|n| find_parent_component(n)) else {
            return false;
        };

        std::rc::Rc::ptr_eq(&s.source_file, &o.source_file) && s.text_range() == o.text_range()
    }

    pub fn contains_offset(&self, offset: TextSize) -> bool {
        self.with_element_node(|node| {
            node.parent().is_some_and(|n| n.text_range().contains(offset))
        })
    }
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

pub fn find_element_with_decoration(element: &syntax_nodes::Element) -> SyntaxNode {
    let this_node: SyntaxNode = element.clone().into();
    element
        .parent()
        .and_then(|p| match p.kind() {
            SyntaxKind::SubElement => p.parent().map(|gp| {
                if gp.kind() == SyntaxKind::ConditionalElement
                    || gp.kind() == SyntaxKind::RepeatedElement
                {
                    gp
                } else {
                    p
                }
            }),
            _ => Some(this_node.clone()),
        })
        .unwrap_or(this_node)
}

pub fn find_parent_component(node: &SyntaxNode) -> Option<SyntaxNode> {
    let mut current = Some(node.clone());
    while let Some(p) = current {
        if matches!(p.kind(), SyntaxKind::Component) {
            return Some(p);
        }
        current = p.parent();
    }
    None
}
