// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
    Parse the contents of builtins.slint and fill the builtin type registry
*/

use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::expression_tree::{BuiltinFunction, Expression};
use crate::langtype::{
    BuiltinElement, BuiltinPropertyDefault, BuiltinPropertyInfo, BuiltinStruct, DefaultSizeBinding,
    ElementType, Function, NativeClass, Type,
};
use crate::object_tree::{self, *};
use crate::parser::{SyntaxKind, SyntaxNode, identifier_text, syntax_nodes};
use crate::typeregister::TypeRegister;

/// Parse the contents of builtins.slint and fill the builtin type registry
/// `register` is the register to fill with the builtin types.
/// At this point, it really should already contain the basic Types (string, int, ...)
pub(crate) fn load_builtins(
    register: &mut TypeRegister,
    symbol_counters: &Rc<crate::symbol_counters::SymbolCounters>,
) {
    let mut diag = crate::diagnostics::BuildDiagnostics::default();
    let node = crate::parser::parse(include_str!("builtins.slint").into(), None, &mut diag);
    if !diag.is_empty() {
        let vec = diag.to_string_vec();
        #[cfg(feature = "display-diagnostics")]
        diag.print();
        panic!("Error parsing the builtin elements: {vec:?}");
    }

    assert_eq!(node.kind(), crate::parser::SyntaxKind::Document);

    // A mistyped annotation key would otherwise be silently ignored.
    const ANNOTATION_KEYS: [&str; 10] = [
        "accepts_focus",
        "builtin_struct",
        "can_be_declared_without_children_slot",
        "constexpr",
        "default_size_binding",
        "disallow_global_types_as_child_elements",
        "fake",
        "is_internal",
        "is_non_item_type",
        "shadowable",
    ];
    if cfg!(debug_assertions) {
        for token in node.node.descendants_with_tokens().filter_map(|t| t.into_token()) {
            if token.kind() == SyntaxKind::Comment
                && let Some(rest) = token.text().strip_prefix("//-")
            {
                let key = rest.trim_end().split(':').next().unwrap();
                assert!(
                    ANNOTATION_KEYS.contains(&key),
                    "unknown annotation `//-{key}` in builtins.slint"
                );
            }
        }
    }

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

                    if member_annotation(&p, "constexpr") {
                        info.property_visibility = PropertyVisibility::Constexpr;
                    } else if member_annotation(&p, "fake") {
                        info.property_visibility = PropertyVisibility::Fake;
                    }

                    info.set_docs(docs::doc_comment(&p));
                    info.shadowable = member_annotation(&p, "shadowable");

                    if let Some(e) = p.BindingExpression() {
                        assert!(!info.shadowable, "shadowable property {id}::{prop_name} can't have a default value as it would end up on the shadowing declaration");
                        let ty = info.ty.clone();
                        info.default_value =
                            BuiltinPropertyDefault::Expr(compiled(e, register, ty, symbol_counters));
                    }

                    (prop_name, info)
                })
                .chain(e.CallbackDeclaration().map(|s| {
                    let mut info = BuiltinPropertyInfo::new(Type::Callback(Arc::new(Function{
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
                    info.set_docs(docs::doc_comment(&s));
                    info.shadowable = member_annotation(&s, "shadowable");
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
            .map(|x| x.unwrap().parse::<BuiltinStruct>().unwrap());
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
            let mut info = match builtin_function_body(&f, &id, &name) {
                Some(function) => {
                    // The BuiltinFunction type prepends implicit ElementReference arguments.
                    let ty = function.ty();
                    let implicit = ty.args.len().saturating_sub(args.len());
                    debug_assert!(
                        ty.args.len() >= args.len()
                            && ty.args[..implicit]
                                .iter()
                                .all(|t| matches!(t, Type::ElementReference))
                            && ty.args[implicit..] == args[..]
                            && ty.return_type == return_type,
                        "the declared signature of {id}::{name} doesn't match {function:?}: {ty:?}"
                    );
                    let mut merged = (*ty).clone();
                    merged.arg_names = std::iter::repeat_n(SmolStr::default(), implicit)
                        .chain(arg_names)
                        .collect();
                    let mut info = BuiltinPropertyInfo::from(function);
                    info.ty = Type::Function(Arc::new(merged));
                    info
                }
                None => BuiltinPropertyInfo::new(Type::Function(
                    Function { return_type, args, arg_names }.into(),
                )),
            };
            info.set_docs(docs::doc_comment(&f));
            info.shadowable = member_annotation(&f, "shadowable");
            (name, info)
        }));

        // NativeClass is not Send yet; the Arc is for the shared langtype graph.
        #[allow(clippy::arc_with_non_send_sync)]
        let mut builtin = BuiltinElement::new(Arc::new(n));
        builtin.is_global = matches!(base, Base::Global);
        let properties = &mut builtin.properties;
        if let Base::NativeParent(parent) = &base {
            properties.extend(parent.properties.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        properties
            .extend(builtin.native_class.properties.iter().map(|(k, v)| (k.clone(), v.clone())));
        let entries = docs::element_doc_entries(&c, &e, &mut diag.borrow_mut());
        let parent_builtin = match &base {
            Base::NativeParent(p) => Some(p.as_ref()),
            _ => None,
        };
        // Assemble docs as [description, inherited parent body, own body].
        // docs[0] is always the description so children can skip it
        // with `parent.docs[1..]`.
        builtin.docs = docs::assemble(entries, parent_builtin);

        builtin.slint_sc = matches!(
            builtin.docs.first(),
            Some(crate::doc_comments::ElementDocEntry::Text(desc)) if has_sc_marker(desc)
        ) || matches!(&base, Base::NativeParent(p) if p.slint_sc);

        builtin.disallow_global_types_as_child_elements =
            parse_annotation("disallow_global_types_as_child_elements", &e).is_some();
        builtin.is_non_item_type = parse_annotation("is_non_item_type", &e).is_some();
        builtin.is_internal = parse_annotation("is_internal", &e).is_some();
        builtin.can_be_declared_without_children_slot =
            parse_annotation("can_be_declared_without_children_slot", &e).is_some();
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
    symbol_counters: &Rc<crate::symbol_counters::SymbolCounters>,
) -> Expression {
    let mut diag = crate::diagnostics::BuildDiagnostics::default();
    let mut ctx =
        crate::lookup::LookupCtx::empty_context(type_register, &mut diag, symbol_counters.clone());
    ctx.property_type = ty.clone();
    ctx.expected_type = ty.clone();
    let e = Expression::from_binding_expression_node(node.clone().into(), &mut ctx)
        .maybe_convert_to(ty, &node, ctx.diag, &ctx.symbol_counters);
    if diag.has_errors() {
        let vec = diag.to_string_vec();
        #[cfg(feature = "display-diagnostics")]
        diag.print();
        panic!("Error parsing the builtin elements: {vec:?}");
    }
    e
}

/// Return true when the member declaration is preceded by a `//-key` comment.
fn member_annotation(node: &SyntaxNode, key: &str) -> bool {
    let mut cursor = node.node.prev_sibling_or_token();
    while let Some(cur) = cursor {
        match cur.kind() {
            SyntaxKind::Whitespace => {}
            SyntaxKind::Comment => {
                if cur.as_token().unwrap().text().trim_end().strip_prefix("//-") == Some(key) {
                    return true;
                }
            }
            _ => return false,
        }
        cursor = cur.prev_sibling_or_token();
    }
    false
}

/// Return the [`BuiltinFunction`] named by a member function's body, like
/// `function start() { BuiltinFunction.StartTimer }`. `None` for an empty body.
fn builtin_function_body(
    f: &syntax_nodes::Function,
    id: &SmolStr,
    name: &SmolStr,
) -> Option<BuiltinFunction> {
    let expr = f.CodeBlock()?.Expression().next()?;
    let function = expr.QualifiedName().and_then(|qn| {
        match QualifiedTypeName::from_node(qn).members.as_slice() {
            [namespace, variant] if namespace == "BuiltinFunction" => {
                variant.parse::<BuiltinFunction>().ok()
            }
            _ => None,
        }
    });
    let Some(function) = function else {
        panic!(
            "the body of {id}::{name} must name the BuiltinFunction variant that implements it, like `BuiltinFunction.StartTimer`"
        )
    };
    Some(function)
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

/// Check for standalone `\sc` marker in a doc string, ensuring it is not
/// followed by an alphanumeric or underscore character (avoids matching
/// `\score`, `\scale`, etc.).
pub(crate) fn has_sc_marker(doc: &str) -> bool {
    doc.match_indices("\\sc").any(|(start, _)| {
        let end = start + 3;
        match doc.as_bytes().get(end).copied() {
            None => true,
            Some(b) => !b.is_ascii_alphanumeric() && b != b'_',
        }
    })
}

use crate::doc_comments as docs;
