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

    /// List of elements that are not attached to the root anymore because they have been
    /// optimized away, but their properties may still be in use
    pub optimized_elements: RefCell<Vec<Rc<RefCell<Element>>>>,

    /// Map of resources to embed in the generated binary, indexed by their absolute path on
    /// disk on the build system and valued by a unique integer id, that can be used by the
    /// generator for symbol generation.
    pub embedded_file_resources: RefCell<HashMap<String, usize>>,

    /// LayoutConstraints
    pub layout_constraints: RefCell<LayoutConstraints>,
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
            ..Default::default()
        }
    }

    pub fn find_element_by_id(&self, name: &str) -> Option<Rc<RefCell<Element>>> {
        pub fn find_element_by_id_recursive(
            e: &Rc<RefCell<Element>>,
            name: &str,
        ) -> Option<Rc<RefCell<Element>>> {
            if e.borrow().id == name {
                return Some(e.clone());
            }
            for x in &e.borrow().children {
                if let Some(x) = find_element_by_id_recursive(x, name) {
                    return Some(x);
                }
            }
            None
        }
        find_element_by_id_recursive(&self.root_element, name)
    }
}

#[derive(Clone, Debug, Default)]
pub struct PropertyDeclaration {
    pub property_type: Type,
    pub type_location: crate::diagnostics::Span,
}

/// An Element is an instentation of a Component
#[derive(Default, Debug)]
pub struct Element {
    /// The id as named in the original .60 file.
    ///
    /// Note that it can only be used for lookup before inlining.
    /// After inlining there can be duplicated id in the component.
    /// The id are then re-assigned unique id in the assign_id pass
    pub id: String,
    //pub base: QualifiedTypeName,
    pub base_type: crate::typeregister::Type,
    /// Currently contains also the signals. FIXME: should that be changed?
    pub bindings: HashMap<String, Expression>,
    pub children: Vec<Rc<RefCell<Element>>>,

    pub property_declarations: HashMap<String, PropertyDeclaration>,

    /// The AST node, if available
    pub node: Option<SyntaxNode>,
}

impl Element {
    pub fn from_node(
        node: SyntaxNode,
        id: String,
        diag: &mut Diagnostics,
        tr: &TypeRegister,
    ) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Element);
        let base =
            QualifiedTypeName::from_node(node.child_node(SyntaxKind::QualifiedName).unwrap());
        let mut r = Element {
            id,
            base_type: tr.lookup(&base.to_string()),
            node: Some(node.clone()),
            ..Default::default()
        };
        if !r.base_type.is_object_type() {
            diag.push_error(
                format!("Unknown type {}", base),
                node.child_node(SyntaxKind::QualifiedName).unwrap().span(),
            );
            return r;
        }

        for prop_decl in node.children().filter(|n| n.kind() == SyntaxKind::PropertyDeclaration) {
            let qualified_type_node = prop_decl
                .children()
                .filter(|n| n.kind() == SyntaxKind::QualifiedName)
                .nth(0)
                .unwrap();
            let type_span = qualified_type_node.span();
            let qualified_type = QualifiedTypeName::from_node(qualified_type_node);

            let prop_type = tr.lookup_qualified(&qualified_type.members);

            match prop_type {
                Type::Invalid => {
                    diag.push_error(
                        format!("Unknown property type '{}'", qualified_type.to_string()),
                        type_span.clone(),
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
            if !matches!(r.lookup_property(&prop_name), Type::Invalid) {
                diag.push_error(
                    format!("Cannot override property '{}'", prop_name),
                    prop_name_token.span(),
                )
            }

            r.property_declarations.insert(
                prop_name.clone(),
                PropertyDeclaration { property_type: prop_type, type_location: type_span },
            );

            if let Some(csn) = prop_decl.child_node(SyntaxKind::BindingExpression) {
                if r.bindings.insert(prop_name, Expression::Uncompiled(csn)).is_some() {
                    diag.push_error("Duplicated property binding".into(), prop_name_token.span());
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
                        Type::Invalid => format!("Unknown property {} in {}", name, base),
                        Type::Signal => format!("'{}' is a signal. Use `=>` to connect", name),
                        _ => format!("Cannot assing to {} in {}", name, base),
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
                    format!("'{}' is not a signal in {}", name, base),
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
            r.property_declarations.insert(
                name,
                PropertyDeclaration { property_type: Type::Signal, type_location: sig_decl.span() },
            );
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
            .map(|decl| decl.property_type)
            .unwrap_or_else(|| self.base_type.lookup_property(name))
    }

    /// Return the Span of this element in the AST for error reporting
    pub fn span(&self) -> crate::diagnostics::Span {
        self.node.as_ref().map(|n| n.span()).unwrap_or_default()
    }
}

#[derive(Default, Debug, Clone)]
pub struct QualifiedTypeName {
    members: Vec<String>,
}

impl QualifiedTypeName {
    pub fn from_node(node: SyntaxNode) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::QualifiedName);
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

#[derive(Default, Clone, Debug)]
pub struct LayoutConstraints(Vec<LayoutConstraint>);

#[derive(Default, Clone, Debug)]
struct LayoutConstraint {
    // Element, property name
    terms: Vec<(Rc<RefCell<Element>>, String)>,
}
