/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

/*!
    Parse the contents of builtins.60 and fill the builtin type registery
*/

use std::rc::Rc;
use std::{cell::RefCell, collections::HashMap};

use crate::{expression_tree::Expression, object_tree::Element, typeregister::TypeRegister};
use crate::{
    langtype::{BuiltinElement, NativeClass, Type},
    parser::SyntaxNode,
};
use crate::{object_tree, object_tree::QualifiedTypeName, parser::identifier_text};
use crate::{
    object_tree::Component,
    parser::{syntax_nodes, SyntaxKind},
};

/// Parse the contents of builtins.60 and fill the builtin type registery
/// `register` is the register to fill with the builtin types.
/// At this point, it really should already contain the basic Types (string, int, ...)
pub fn load_builtins(register: &mut TypeRegister) {
    let (node, mut diag) = crate::parser::parse(include_str!("builtins.60").into(), None);
    if !diag.is_empty() {
        let vec = diag.to_string_vec();
        #[cfg(feature = "display-diagnostics")]
        diag.print();
        panic!("Error parsing the builtin elements: {:?}", vec);
    }

    assert_eq!(node.kind(), crate::parser::SyntaxKind::Document);
    let doc: syntax_nodes::Document = node.into();

    // parse structs
    for s in doc.StructDeclaration().chain(doc.ExportsList().flat_map(|e| e.StructDeclaration())) {
        let external_name = identifier_text(&s.DeclaredIdentifier()).unwrap();
        let mut ty = object_tree::type_struct_from_node(s.ObjectType(), &mut diag, register);
        if let Type::Object { name, .. } = &mut ty {
            *name = Some(
                parse_annotation("name", &s.ObjectType().node)
                    .map_or_else(|| external_name.clone(), |s| s.unwrap())
                    .to_owned(),
            );
        } else {
            unreachable!()
        }
        register.insert_type_with_name(ty, external_name);
    }

    let mut natives = HashMap::<String, Rc<BuiltinElement>>::new();

    let exports = doc
        .ExportsList()
        .flat_map(|e| {
            e.Component()
                .map(|x| {
                    let x = identifier_text(&x.DeclaredIdentifier()).unwrap();
                    (x.clone(), x)
                })
                .into_iter()
                .chain(e.ExportSpecifier().map(|e| {
                    (
                        identifier_text(&e.ExportIdentifier()).unwrap(),
                        identifier_text(&e.ExportName().unwrap()).unwrap(),
                    )
                }))
        })
        .collect::<HashMap<_, _>>();

    for c in doc.Component().chain(doc.ExportsList().filter_map(|e| e.Component())) {
        let id = identifier_text(&c.DeclaredIdentifier()).unwrap();
        let e = c.Element();
        let diag = RefCell::new(&mut diag);
        let mut n = NativeClass::new_with_properties(
            &id,
            e.PropertyDeclaration()
                .map(|p| {
                    (
                        identifier_text(&p.DeclaredIdentifier()).unwrap(),
                        object_tree::type_from_node(p.Type(), *diag.borrow_mut(), register),
                    )
                })
                .chain(e.SignalDeclaration().map(|s| {
                    (
                        identifier_text(&s.DeclaredIdentifier()).unwrap(),
                        Type::Signal {
                            args: s
                                .Type()
                                .map(|a| {
                                    object_tree::type_from_node(a, *diag.borrow_mut(), register)
                                })
                                .collect(),
                        },
                    )
                })),
        );
        n.cpp_type = parse_annotation("cpp_type", &e.node).map(|x| x.unwrap().to_owned());
        n.rust_type_constructor =
            parse_annotation("rust_type_constructor", &e.node).map(|x| x.unwrap().to_owned());
        let global = if let Some(base) = e.QualifiedName() {
            let base = QualifiedTypeName::from_node(base).to_string();
            if base != "_" {
                n.parent = Some(natives.get(&base).unwrap().native_class.clone())
            };
            false
        } else {
            true
        };
        let mut builtin = BuiltinElement::new(Rc::new(n));
        builtin.is_global = global;
        builtin.default_bindings.extend(e.PropertyDeclaration().filter_map(|p| {
            Some((
                identifier_text(&p.DeclaredIdentifier())?,
                compiled(p.BindingExpression()?, register),
            ))
        }));
        builtin.disallow_global_types_as_child_elements =
            parse_annotation("disallow_global_types_as_child_elements", &e.node).is_some();
        builtin.is_non_item_type = parse_annotation("is_non_item_type", &e.node).is_some();
        builtin.additional_accepted_child_types = e
            .SubElement()
            .map(|s| {
                let a = identifier_text(&s.Element().QualifiedName().unwrap()).unwrap();
                let t = natives[&a].clone();
                (a, Type::Builtin(t))
            })
            .collect();
        if let Some(builtin_name) = exports.get(&id) {
            if !global {
                register
                    .insert_type_with_name(Type::Builtin(Rc::new(builtin)), builtin_name.clone());
            } else {
                let glob = Rc::new(Component {
                    id: builtin_name.clone(),
                    root_element: Rc::new(RefCell::new(Element {
                        base_type: Type::Builtin(Rc::new(builtin)),
                        ..Default::default()
                    })),
                    ..Default::default()
                });
                glob.root_element.borrow_mut().enclosing_component = Rc::downgrade(&glob);
                register.insert_type(Type::Component(glob));
            }
        } else {
            assert!(builtin.default_bindings.is_empty()); // because they are not taken from if we inherit from it
            assert!(builtin.additional_accepted_child_types.is_empty());
            natives.insert(id, Rc::new(builtin));
        }
    }

    register.property_animation_type = Type::Builtin(natives.remove("PropertyAnimation").unwrap());

    if !diag.is_empty() {
        let vec = diag.to_string_vec();
        #[cfg(feature = "display-diagnostics")]
        diag.print();
        panic!("Error loading the builtin elements: {:?}", vec);
    }
}

/// Compile an expression, knowing that the expression is basic
fn compiled(node: syntax_nodes::BindingExpression, type_register: &TypeRegister) -> Expression {
    let mut diag = crate::diagnostics::BuildDiagnostics::default();
    let e = Expression::from_binding_expression_node(
        node.into(),
        &mut crate::passes::resolving::LookupCtx::empty_context(type_register, &mut diag),
    );
    if diag.has_error() {
        let vec = diag.to_string_vec();
        #[cfg(feature = "display-diagnostics")]
        diag.print();
        panic!("Error parsing the builtin elements: {:?}", vec);
    }
    e
}

/// Find out if there are comments that starts with `//-key` and returns `None`
/// if no annotation with this key is found, or `Some(None)` if it is found without a value
/// or `Some(Some(value))` if there is a `//-key:value`  match
fn parse_annotation(key: &str, node: &SyntaxNode) -> Option<Option<String>> {
    for x in node.children_with_tokens() {
        if x.kind() == SyntaxKind::Comment {
            if let Some(comment) = x
                .as_token()
                .unwrap()
                .text()
                .as_str()
                .strip_prefix("//-")
                .and_then(|x| x.strip_prefix(key))
            {
                if comment.is_empty() {
                    return Some(None);
                }
                if let Some(comment) = comment.strip_prefix(":") {
                    return Some(Some(comment.to_owned()));
                }
            }
        }
    }
    None
}
