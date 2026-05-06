// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
    Parse the contents of builtins.slint and fill the builtin type registry
*/

use smol_str::{SmolStr, ToSmolStr};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::expression_tree::Expression;
use crate::langtype::{
    BuiltinElement, BuiltinElementDocEntry, BuiltinPrivateStruct, BuiltinPropertyDefault,
    BuiltinPropertyInfo, DefaultSizeBinding, ElementType, Function, NativeClass, Type,
};
use crate::object_tree::{self, *};
use crate::parser::{SyntaxKind, SyntaxNode, identifier_text, syntax_nodes};
use crate::typeregister::TypeRegister;

/// Parse the contents of builtins.slint and fill the builtin type registry
/// `register` is the register to fill with the builtin types.
/// At this point, it really should already contain the basic Types (string, int, ...)
pub(crate) fn load_builtins(register: &mut TypeRegister) {
    let mut diag = crate::diagnostics::BuildDiagnostics::default();
    let node = crate::parser::parse(include_str!("builtins.slint").into(), None, &mut diag);
    if !diag.is_empty() {
        let vec = diag.to_string_vec();
        #[cfg(feature = "display-diagnostics")]
        diag.print();
        panic!("Error parsing the builtin elements: {vec:?}");
    }

    assert_eq!(node.kind(), crate::parser::SyntaxKind::Document);
    let doc: syntax_nodes::Document = node.into();

    let mut natives = HashMap::<SmolStr, Rc<BuiltinElement>>::new();

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
                .filter(|p| p.TwoWayBinding().is_none()) // aliases are handled further down
                .map(|p| {
                    let prop_name = identifier_text(&p.DeclaredIdentifier()).unwrap();

                    let mut info = BuiltinPropertyInfo::new(object_tree::type_from_node(
                        p.Type().unwrap(),
                        *diag.borrow_mut(),
                        register,
                    ));

                    info.property_visibility = PropertyVisibility::Private;

                    for token in p.children_with_tokens() {
                        if token.kind() != SyntaxKind::Identifier {
                            continue;
                        }
                        match (token.as_token().unwrap().text(), info.property_visibility) {
                            ("in", PropertyVisibility::Private) => {
                                info.property_visibility = PropertyVisibility::Input
                            }
                            ("out", PropertyVisibility::Private) => {
                                info.property_visibility = PropertyVisibility::Output
                            }
                            ("in-out", PropertyVisibility::Private) => {
                                info.property_visibility = PropertyVisibility::InOut
                            }
                            ("property", _) => (),
                            _ => unreachable!("invalid property keyword when parsing builtin file for property {id}::{prop_name}"),
                        }
                    }

                    info.docs = docs::doc_comment(&p);

                    if let Some(e) = p.BindingExpression() {
                        let ty = info.ty.clone();
                        info.default_value = BuiltinPropertyDefault::Expr(compiled(e, register, ty));
                    }

                    (prop_name, info)
                })
                .chain(e.CallbackDeclaration().map(|s| {
                    let mut info = BuiltinPropertyInfo::new(Type::Callback(Rc::new(Function{
                        args: s
                            .CallbackDeclarationParameter()
                            .map(|a| {
                                object_tree::type_from_node(a.Type(), *diag.borrow_mut(), register)
                            })
                            .collect(),
                        return_type: s.ReturnType().map(|a| {
                            object_tree::type_from_node(
                                a.Type(),
                                *diag.borrow_mut(),
                                register,
                            )
                        }).unwrap_or(Type::Void),
                        arg_names: s
                            .CallbackDeclarationParameter()
                            .map(|a| a.DeclaredIdentifier().and_then(|x| identifier_text(&x)).unwrap_or_default())
                            .collect()
                    })));
                    info.docs = docs::doc_comment(&s);
                    (identifier_text(&s.DeclaredIdentifier()).unwrap(), info)
                }))
        );
        n.deprecated_aliases = e
            .PropertyDeclaration()
            .flat_map(|p| {
                if let Some(twb) = p.TwoWayBinding() {
                    let alias_name = identifier_text(&p.DeclaredIdentifier()).unwrap();
                    let alias_target = identifier_text(&twb.Expression().QualifiedName().expect(
                        "internal error: built-in aliases can only be declared within the type",
                    ))
                    .unwrap();
                    Some((alias_name, alias_target))
                } else {
                    None
                }
            })
            .collect();
        n.builtin_struct = parse_annotation("builtin_struct", &e)
            .map(|x| x.unwrap().parse::<BuiltinPrivateStruct>().unwrap());
        enum Base {
            None,
            Global,
            NativeParent(Rc<BuiltinElement>),
        }
        let base = if c.child_text(SyntaxKind::Identifier).is_some_and(|t| t == "global") {
            Base::Global
        } else if let Some(base) = e.QualifiedName() {
            let base = QualifiedTypeName::from_node(base).to_smolstr();
            let base = natives.get(&base).unwrap().clone();
            // because they are not taken from if we inherit from it
            assert!(
                base.additional_accepted_child_types.is_empty() && !base.additional_accept_self
            );
            n.parent = Some(base.native_class.clone());
            Base::NativeParent(base)
        } else {
            Base::None
        };

        n.properties.extend(e.Function().map(|f| {
            let name = identifier_text(&f.DeclaredIdentifier()).unwrap();
            let return_type = f.ReturnType().map_or(Type::Void, |p| {
                object_tree::type_from_node(p.Type(), *diag.borrow_mut(), register)
            });
            let mut args = Vec::new();
            let mut arg_names = Vec::new();
            for a in f.ArgumentDeclaration() {
                args.push(object_tree::type_from_node(a.Type(), *diag.borrow_mut(), register));
                arg_names.push(identifier_text(&a.DeclaredIdentifier()).unwrap_or_default());
            }
            let mut info = BuiltinPropertyInfo::new(Type::Function(
                Function { return_type, args, arg_names }.into(),
            ));
            info.docs = docs::doc_comment(&f);
            (name, info)
        }));

        let mut builtin = BuiltinElement::new(Rc::new(n));
        builtin.is_global = matches!(base, Base::Global);
        let properties = &mut builtin.properties;
        if let Base::NativeParent(parent) = &base {
            properties.extend(parent.properties.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        properties
            .extend(builtin.native_class.properties.iter().map(|(k, v)| (k.clone(), v.clone())));
        let (description, body) = docs::element_doc_entries(&c, &e);
        let parent_builtin = match &base {
            Base::NativeParent(p) => Some(p.as_ref()),
            _ => None,
        };
        // Assemble docs as [description, inherited parent body, own body].
        // docs[0] is always the description so children can skip it
        // with `parent.docs[1..]`.
        builtin.docs = docs::assemble(description, parent_builtin, body);

        builtin.disallow_global_types_as_child_elements =
            parse_annotation("disallow_global_types_as_child_elements", &e).is_some();
        builtin.is_non_item_type = parse_annotation("is_non_item_type", &e).is_some();
        builtin.is_internal = parse_annotation("is_internal", &e).is_some();
        builtin.accepts_focus = parse_annotation("accepts_focus", &e).is_some();
        builtin.default_size_binding = parse_annotation("default_size_binding", &e)
            .map(|size_type| match size_type.as_deref() {
                Some("expands_to_parent_geometry") => DefaultSizeBinding::ExpandsToParentGeometry,
                Some("implicit_size") => DefaultSizeBinding::ImplicitSize,
                other => panic!("invalid default size binding {other:?}"),
            })
            .unwrap_or(DefaultSizeBinding::None);
        builtin.additional_accepted_child_types = e
            .SubElement()
            .filter_map(|s| {
                let a = identifier_text(&s.Element().QualifiedName().unwrap()).unwrap();
                if a == builtin.native_class.class_name {
                    builtin.additional_accept_self = true;
                    None
                } else {
                    let t = natives[&a].clone();
                    Some((a, t))
                }
            })
            .collect();
        if let Some(builtin_name) = exports.get(&id) {
            if !matches!(&base, Base::Global) {
                builtin.name.clone_from(builtin_name);
                register.add_builtin(Rc::new(builtin));
            } else {
                let glob = Rc::new(Component {
                    id: builtin_name.clone(),
                    root_element: Rc::new(RefCell::new(Element {
                        base_type: ElementType::Builtin(Rc::new(builtin)),
                        ..Default::default()
                    })),
                    ..Default::default()
                });
                glob.root_element.borrow_mut().enclosing_component = Rc::downgrade(&glob);
                register.add(glob);
            }
        } else {
            natives.insert(id, Rc::new(builtin));
        }
    }

    register.property_animation_type =
        ElementType::Builtin(natives.remove("PropertyAnimation").unwrap());

    register.empty_type = ElementType::Builtin(natives.remove("Empty").unwrap());

    if !diag.is_empty() {
        let vec = diag.to_string_vec();
        #[cfg(feature = "display-diagnostics")]
        diag.print();
        panic!("Error loading the builtin elements: {vec:?}");
    }
}

/// Compile an expression, knowing that the expression is basic (does not have lookup to other things)
fn compiled(
    node: syntax_nodes::BindingExpression,
    type_register: &TypeRegister,
    ty: Type,
) -> Expression {
    let mut diag = crate::diagnostics::BuildDiagnostics::default();
    let mut ctx = crate::lookup::LookupCtx::empty_context(type_register, &mut diag);
    ctx.property_type = ty.clone();
    let e = Expression::from_binding_expression_node(node.clone().into(), &mut ctx)
        .maybe_convert_to(ty, &node, &mut diag);
    if diag.has_errors() {
        let vec = diag.to_string_vec();
        #[cfg(feature = "display-diagnostics")]
        diag.print();
        panic!("Error parsing the builtin elements: {vec:?}");
    }
    e
}

/// Find out if there are comments that starts with `//-key` and returns `None`
/// if no annotation with this key is found, or `Some(None)` if it is found without a value
/// or `Some(Some(value))` if there is a `//-key:value`  match
fn parse_annotation(key: &str, node: &SyntaxNode) -> Option<Option<SmolStr>> {
    for x in node.children_with_tokens() {
        if x.kind() == SyntaxKind::Comment
            && let Some(comment) = x
                .as_token()
                .unwrap()
                .text()
                .strip_prefix("//-")
                .and_then(|x| x.trim_end().strip_prefix(key))
        {
            if comment.is_empty() {
                return Some(None);
            }
            if let Some(comment) = comment.strip_prefix(':') {
                return Some(Some(comment.into()));
            }
        }
    }
    None
}

/// Extract `///` doc comments from the syntax tree of `builtins.slint`.
mod docs {
    use super::*;

    /// Walk backwards across sibling tokens/nodes collecting consecutive
    /// `///` doc comment lines immediately before `anchor`. Returns the
    /// concatenated text with the `/// ` prefix stripped, or `None` if
    /// no doc comment was present.
    fn collect_before(anchor: &SyntaxNode) -> Option<String> {
        let mut lines = Vec::new();
        let mut cursor = anchor.node.prev_sibling_or_token();
        while let Some(cur) = cursor {
            match cur.kind() {
                SyntaxKind::Whitespace => {}
                SyntaxKind::Comment => {
                    let text = cur.as_token().unwrap().text().to_string();
                    if text.starts_with("///") {
                        lines.push(text);
                    } else if text.starts_with("//") {
                        // Skip regular comments and //-annotations.
                    } else {
                        break;
                    }
                }
                SyntaxKind::ExportsList => {
                    // Doc comments may sit inside a preceding `export { ... }` list.
                    if let Some(list) = cur.as_node() {
                        let mut last = list.last_child_or_token();
                        while let Some(child) = last {
                            match child.kind() {
                                SyntaxKind::Whitespace => {}
                                SyntaxKind::Comment => {
                                    let t = child.as_token().unwrap().text().to_string();
                                    if t.starts_with("///") {
                                        lines.push(t);
                                    } else if t.starts_with("//") {
                                        // skip
                                    } else {
                                        break;
                                    }
                                }
                                _ => break,
                            }
                            last = child.prev_sibling_or_token();
                        }
                    }
                    break;
                }
                _ => break,
            }
            cursor = cur.prev_sibling_or_token();
        }
        if lines.is_empty() {
            return None;
        }
        lines.reverse();
        Some(
            lines
                .iter()
                .map(|t| t.strip_prefix("/// ").or_else(|| t.strip_prefix("///")).unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    /// Extract the `///` doc comment before a syntax node. Also checks
    /// above the enclosing `ExportsList` when the node is inside one.
    pub(super) fn doc_comment(anchor: &SyntaxNode) -> Option<String> {
        if let Some(doc) = collect_before(anchor) {
            return Some(doc);
        }
        if let Some(parent) = anchor.parent()
            && parent.kind() == SyntaxKind::ExportsList
        {
            return collect_before(&parent);
        }
        None
    }

    /// Extract the `///` description before the component and the ordered
    /// body entries (`//!` text and member references) from inside it.
    pub(super) fn element_doc_entries(
        component: &SyntaxNode,
        element: &syntax_nodes::Element,
    ) -> (Option<String>, Vec<BuiltinElementDocEntry>) {
        let description = doc_comment(component);

        let mut entries = Vec::new();
        let mut section_lines: Vec<String> = Vec::new();
        let flush_section = |lines: &mut Vec<String>, entries: &mut Vec<BuiltinElementDocEntry>| {
            if !lines.is_empty() {
                entries.push(BuiltinElementDocEntry::Text(lines.join("\n")));
                lines.clear();
            }
        };

        for child in element.children_with_tokens() {
            match child.kind() {
                SyntaxKind::Comment => {
                    if let Some(t) = child.as_token() {
                        let text = t.text();
                        if let Some(content) =
                            text.strip_prefix("//! ").or_else(|| text.strip_prefix("//!"))
                        {
                            section_lines.push(content.to_string());
                        }
                    }
                }
                SyntaxKind::PropertyDeclaration => {
                    let p = syntax_nodes::PropertyDeclaration::from(child.into_node().unwrap());
                    if p.TwoWayBinding().is_some() {
                        continue;
                    }
                    flush_section(&mut section_lines, &mut entries);
                    let name = identifier_text(&p.DeclaredIdentifier()).unwrap();
                    entries.push(BuiltinElementDocEntry::Member(name));
                }
                SyntaxKind::CallbackDeclaration => {
                    let cb = syntax_nodes::CallbackDeclaration::from(child.into_node().unwrap());
                    if cb.TwoWayBinding().is_some() {
                        continue;
                    }
                    flush_section(&mut section_lines, &mut entries);
                    let name = identifier_text(&cb.DeclaredIdentifier()).unwrap();
                    entries.push(BuiltinElementDocEntry::Member(name));
                }
                SyntaxKind::Function => {
                    let f = syntax_nodes::Function::from(child.into_node().unwrap());
                    flush_section(&mut section_lines, &mut entries);
                    let name = identifier_text(&f.DeclaredIdentifier()).unwrap();
                    entries.push(BuiltinElementDocEntry::Member(name));
                }
                _ => {}
            }
        }
        flush_section(&mut section_lines, &mut entries);
        (description, entries)
    }

    /// Assemble the final doc entries for an element:
    /// `[description, inherited parent body, own body]`.
    pub(super) fn assemble(
        description: Option<String>,
        parent: Option<&BuiltinElement>,
        body: Vec<BuiltinElementDocEntry>,
    ) -> Vec<BuiltinElementDocEntry> {
        let desc = description.unwrap_or_default();
        let skip_inherited = desc.contains("\\skip_inherited");

        let mut result = vec![BuiltinElementDocEntry::Text(desc)];
        if !skip_inherited && let Some(parent) = parent {
            result.extend(parent.docs[1..].iter().cloned());
        }
        result.extend(body);
        result
    }
}
