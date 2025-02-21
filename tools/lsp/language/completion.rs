// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore rfind

use crate::common::component_catalog::all_exported_components;
use crate::common::{self, DocumentCache};
use crate::util::{lookup_current_element_type, text_size_to_lsp_position, with_lookup_ctx};

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;
use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::expression_tree::{Callable, Expression};
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::lookup::{LookupCtx, LookupObject, LookupResult, LookupResultCallable};
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken, TextSize};
use i_slint_compiler::typeregister::TypeRegister;
use lsp_types::{
    CompletionClientCapabilities, CompletionItem, CompletionItemKind, InsertTextFormat, Position,
    Range, TextEdit,
};
use smol_str::SmolStr;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(crate) fn completion_at(
    document_cache: &mut DocumentCache,
    token: SyntaxToken,
    offset: TextSize,
    client_caps: Option<&CompletionClientCapabilities>,
) -> Option<Vec<CompletionItem>> {
    let node = token.parent();

    let snippet_support = client_caps
        .and_then(|caps| caps.completion_item.as_ref())
        .and_then(|caps| caps.snippet_support)
        .unwrap_or(false);

    if token.kind() == SyntaxKind::StringLiteral {
        if matches!(node.kind(), SyntaxKind::ImportSpecifier | SyntaxKind::AtImageUrl) {
            return complete_path_in_string(
                token.source_file()?.path(),
                token.text(),
                offset.checked_sub(token.text_range().start())?,
            )
            .map(|mut r| {
                if node.kind() == SyntaxKind::ImportSpecifier && !token.text().contains('/') {
                    let mut c =
                        CompletionItem::new_simple("std-widgets.slint".into(), String::new());

                    c.kind = Some(CompletionItemKind::FILE);
                    r.push(c)
                }
                r
            });
        }
    } else if let Some(element) = syntax_nodes::Element::new(node.clone()) {
        if token.kind() == SyntaxKind::At
            || (token.kind() == SyntaxKind::Identifier
                && token.prev_token().is_some_and(|t| t.kind() == SyntaxKind::At))
        {
            return Some(vec![CompletionItem::new_simple("children".into(), String::new())]);
        }

        let with_snippets = snippet_support && !is_followed_by_brace(&token);
        return resolve_element_scope(element, document_cache, with_snippets).map(|mut r| {
            let is_global = node
                .parent()
                .and_then(|n| n.child_text(SyntaxKind::Identifier))
                .is_some_and(|k| k == "global");

            // add keywords
            r.extend(
                [
                    ("property", "property <${1:int}> ${2:name};"),
                    ("in property", "in property <${1:int}> ${2:name};"),
                    ("in-out property", "in-out property <${1:int}> ${2:name};"),
                    ("out property", "out property <${1:int}> ${2:name};"),
                    ("private property", "private property <${1:int}> ${2:name};"),
                    ("function", "function ${1:name}($2) {\n    $0\n}"),
                    ("public function", "public function ${1:name}($2) {\n    $0\n}"),
                    ("callback", "callback ${1:name}($2);"),
                ]
                .iter()
                .map(|(kw, ins_tex)| {
                    let mut c = CompletionItem::new_simple(kw.to_string(), String::new());
                    c.kind = Some(CompletionItemKind::KEYWORD);
                    with_insert_text(c, ins_tex, snippet_support)
                }),
            );

            if !is_global {
                r.extend(
                    [
                        ("animate", "animate ${1:prop} {\n     $0\n}"),
                        ("changed", "changed ${1:prop} => {$0}"),
                        ("states", "states [\n    $0\n]"),
                        ("for", "for $1 in $2: ${3:Rectangle} {\n    $0\n}"),
                        ("if", "if $1: ${2:Rectangle} {\n    $0\n}"),
                        ("@children", "@children"),
                    ]
                    .iter()
                    .map(|(kw, ins_tex)| {
                        let mut c = CompletionItem::new_simple(kw.to_string(), String::new());
                        c.kind = Some(CompletionItemKind::KEYWORD);
                        with_insert_text(c, ins_tex, snippet_support)
                    }),
                );
            }

            if !is_global && snippet_support {
                add_components_to_import(&token, document_cache, &mut r);
            }

            r
        });
    } else if let Some(n) = syntax_nodes::Binding::new(node.clone()) {
        if let Some(colon) = n.child_token(SyntaxKind::Colon) {
            if offset >= colon.text_range().end() {
                return with_lookup_ctx(document_cache, node, |ctx| {
                    resolve_expression_scope(ctx, document_cache, snippet_support).map(Into::into)
                })?;
            }
        }
        if token.kind() != SyntaxKind::Identifier {
            return None;
        }
        let all =
            resolve_element_scope(syntax_nodes::Element::new(n.parent()?)?, document_cache, false)?;
        return Some(
            all.into_iter()
                .filter(|ce| ce.kind == Some(CompletionItemKind::PROPERTY))
                .collect::<Vec<_>>(),
        );
    } else if let Some(n) = syntax_nodes::TwoWayBinding::new(node.clone()) {
        let double_arrow_range =
            n.children_with_tokens().find(|n| n.kind() == SyntaxKind::DoubleArrow)?.text_range();
        if offset < double_arrow_range.end() {
            return None;
        }
        return with_lookup_ctx(document_cache, node, |ctx| {
            resolve_expression_scope(ctx, document_cache, snippet_support)
        })?;
    } else if let Some(n) = syntax_nodes::CallbackConnection::new(node.clone()) {
        if token.kind() == SyntaxKind::Whitespace || token.kind() == SyntaxKind::FatArrow {
            let ident = n.child_token(SyntaxKind::Identifier)?;
            if offset >= ident.text_range().end()
                && offset <= n.child_token(SyntaxKind::FatArrow)?.text_range().start()
                && ident.text() == "changed"
            {
                return properties_for_changed_callbacks(node, document_cache);
            }
            return None;
        } else if token.kind() != SyntaxKind::Identifier {
            return None;
        }
        let mut parent = n.parent()?;
        let element = loop {
            if let Some(e) = syntax_nodes::Element::new(parent.clone()) {
                break e;
            }
            parent = parent.parent()?;
        };
        let all = resolve_element_scope(element, document_cache, false)?;
        return Some(
            all.into_iter()
                .filter(|ce| ce.kind == Some(CompletionItemKind::METHOD))
                .collect::<Vec<_>>(),
        );
    } else if matches!(
        node.kind(),
        SyntaxKind::Type | SyntaxKind::ArrayType | SyntaxKind::ObjectType | SyntaxKind::ReturnType
    ) {
        return resolve_type_scope(token, document_cache).map(Into::into);
    } else if syntax_nodes::PropertyDeclaration::new(node.clone()).is_some() {
        if token.kind() == SyntaxKind::LAngle {
            return resolve_type_scope(token, document_cache).map(Into::into);
        }
    } else if let Some(n) = syntax_nodes::CallbackDeclaration::new(node.clone()) {
        let paren = n.child_token(SyntaxKind::LParent)?;
        if token.token.text_range().start() >= paren.token.text_range().end() {
            return resolve_type_scope(token, document_cache).map(Into::into);
        }
    } else if matches!(
        node.kind(),
        SyntaxKind::BindingExpression
            | SyntaxKind::CodeBlock
            | SyntaxKind::ReturnStatement
            | SyntaxKind::Expression
            | SyntaxKind::FunctionCallExpression
            | SyntaxKind::SelfAssignment
            | SyntaxKind::ConditionalExpression
            | SyntaxKind::BinaryExpression
            | SyntaxKind::UnaryOpExpression
            | SyntaxKind::Array
            | SyntaxKind::AtGradient
            | SyntaxKind::StringTemplate
            | SyntaxKind::IndexExpression
    ) {
        if token.kind() == SyntaxKind::At
            || (token.kind() == SyntaxKind::Identifier
                && token.prev_token().is_some_and(|t| t.kind() == SyntaxKind::At))
        {
            return Some(
                [
                    ("tr", "tr(\"$1\")"),
                    ("image-url", "image-url(\"$1\")"),
                    ("linear-gradient", "linear-gradient($1)"),
                    ("radial-gradient", "radial-gradient(circle, $1)"),
                ]
                .into_iter()
                .map(|(label, insert)| {
                    with_insert_text(
                        CompletionItem::new_simple(label.into(), String::new()),
                        insert,
                        snippet_support,
                    )
                })
                .collect::<Vec<_>>(),
            );
        }

        return with_lookup_ctx(document_cache, node, |ctx| {
            resolve_expression_scope(ctx, document_cache, snippet_support).map(Into::into)
        })?;
    } else if let Some(q) = syntax_nodes::QualifiedName::new(node.clone()) {
        match q.parent()?.kind() {
            SyntaxKind::Element => {
                // auto-complete the components
                let global_tr = document_cache.global_type_registry();
                let tr = q
                    .source_file()
                    .and_then(|sf| document_cache.get_document_for_source_file(sf))
                    .map(|doc| &doc.local_registry)
                    .unwrap_or(&global_tr);

                let mut result = tr
                    .all_elements()
                    .into_iter()
                    .filter_map(|(k, t)| {
                        match t {
                            ElementType::Component(c) if !c.is_global() => (),
                            ElementType::Builtin(b) if !b.is_internal && !b.is_global => (),
                            _ => return None,
                        };
                        let mut c = CompletionItem::new_simple(k.to_string(), "element".into());
                        c.kind = Some(CompletionItemKind::CLASS);
                        Some(c)
                    })
                    .collect::<Vec<_>>();

                drop(global_tr);

                if snippet_support {
                    add_components_to_import(&token, document_cache, &mut result);
                }

                return Some(result);
            }
            SyntaxKind::Type => {
                return resolve_type_scope(token, document_cache).map(Into::into);
            }
            SyntaxKind::Expression => {
                return with_lookup_ctx(document_cache, node, |ctx| {
                    let it = q.children_with_tokens().filter_map(|t| t.into_token());
                    let mut it = it.skip_while(|t| {
                        t.kind() != SyntaxKind::Identifier && t.token != token.token
                    });
                    let first = it.next();
                    if first.as_ref().map_or(true, |f| f.token == token.token) {
                        return resolve_expression_scope(ctx, document_cache, snippet_support)
                            .map(Into::into);
                    }
                    let first = i_slint_compiler::parser::normalize_identifier(first?.text());
                    let global = i_slint_compiler::lookup::global_lookup();
                    let mut expr_it = global.lookup(ctx, &first)?;
                    let mut has_dot = false;
                    for t in it {
                        has_dot |= t.kind() == SyntaxKind::Dot;
                        if t.token == token.token {
                            break;
                        };
                        if t.kind() != SyntaxKind::Identifier {
                            continue;
                        }
                        has_dot = false;
                        let str = i_slint_compiler::parser::normalize_identifier(t.text());
                        expr_it = expr_it.lookup(ctx, &str)?;
                    }
                    has_dot.then(|| {
                        let mut r = Vec::new();
                        expr_it.for_each_entry(ctx, &mut |str, expr| -> Option<()> {
                            r.push(completion_item_from_expression(str, expr));
                            None
                        });
                        r
                    })
                })?;
            }
            _ => (),
        }
    } else if node.kind() == SyntaxKind::ImportIdentifierList {
        let import = syntax_nodes::ImportSpecifier::new(node.parent()?)?;

        let path = document_cache
            .resolve_import_path(
                Some(&token.into()),
                import.child_text(SyntaxKind::StringLiteral)?.trim_matches('\"'),
            )?
            .0;
        let doc = document_cache.get_document_by_path(&path)?;
        return Some(
            doc.exports
                .iter()
                .map(|(exported_name, _)| CompletionItem {
                    label: exported_name.name.to_string(),
                    ..Default::default()
                })
                .collect(),
        );
    } else if node.kind() == SyntaxKind::Document {
        let mut r: Vec<_> = [
            // the $1 is first in the quote so the filename can be completed before the import names
            ("import", "import { ${2:Component} } from \"${1:std-widgets.slint}\";"),
            ("component", "component ${1:Component} {\n    $0\n}"),
            ("struct", "struct ${1:Name} {\n    $0\n}"),
            ("global", "global ${1:Name} {\n    $0\n}"),
            ("export", "export { $0 }"),
            ("export component", "export component ${1:ExportedComponent} {\n    $0\n}"),
            ("export struct", "export struct ${1:Name} {\n    $0\n}"),
            ("export global", "export global ${1:Name} {\n    $0\n}"),
        ]
        .iter()
        .map(|(kw, ins_tex)| {
            let mut c = CompletionItem::new_simple(kw.to_string(), String::new());
            c.kind = Some(CompletionItemKind::KEYWORD);
            with_insert_text(c, ins_tex, snippet_support)
        })
        .collect();
        if let Some(component) = token
            .prev_sibling_or_token()
            .filter(|x| x.kind() == SyntaxKind::Component)
            .and_then(|x| x.into_node())
        {
            let has_child = |kind| {
                !component.children().find(|n| n.kind() == kind).unwrap().text_range().is_empty()
            };
            if has_child(SyntaxKind::DeclaredIdentifier) && !has_child(SyntaxKind::Element) {
                let mut c = CompletionItem::new_simple("inherits".into(), String::new());
                c.kind = Some(CompletionItemKind::KEYWORD);
                r.push(c)
            }
        }
        return Some(r);
    } else if let Some(c) = syntax_nodes::Component::new(node.clone()) {
        let id_range = c.DeclaredIdentifier().text_range();
        if !id_range.is_empty()
            && offset >= id_range.end()
            && !c
                .children_with_tokens()
                .any(|c| c.as_token().is_some_and(|t| t.text() == "inherits"))
        {
            let mut c = CompletionItem::new_simple("inherits".into(), String::new());
            c.kind = Some(CompletionItemKind::KEYWORD);
            return Some(vec![c]);
        }
    } else if node.kind() == SyntaxKind::State {
        let r: Vec<_> = [("when", "when $1: {\n    $0\n}")]
            .iter()
            .map(|(kw, ins_tex)| {
                let mut c = CompletionItem::new_simple(kw.to_string(), String::new());
                c.kind = Some(CompletionItemKind::KEYWORD);
                with_insert_text(c, ins_tex, snippet_support)
            })
            .collect();
        return Some(r);
    } else if node.kind() == SyntaxKind::PropertyAnimation {
        let global_tr = document_cache.global_type_registry();
        let r = global_tr
            .property_animation_type_for_property(Type::Float32)
            .property_list()
            .into_iter()
            .map(|(k, t)| {
                let mut c = CompletionItem::new_simple(k.to_string(), t.to_string());
                c.kind = Some(CompletionItemKind::PROPERTY);
                if snippet_support {
                    c.insert_text_format = Some(InsertTextFormat::SNIPPET);
                    c.insert_text = Some(format!("{}: $1;", c.label));
                }
                c
            })
            .collect::<Vec<_>>();
        return Some(r);
    } else if node.kind() == SyntaxKind::DeclaredIdentifier {
        let parent = node.parent()?;
        if parent.kind() == SyntaxKind::PropertyChangedCallback {
            return properties_for_changed_callbacks(parent, document_cache);
        }
    } else if node.kind() == SyntaxKind::PropertyChangedCallback {
        if offset > node.child_token(SyntaxKind::Identifier)?.text_range().end() {
            return properties_for_changed_callbacks(node, document_cache);
        }
    }
    None
}

fn with_insert_text(
    mut c: CompletionItem,
    ins_text: &str,
    snippet_support: bool,
) -> CompletionItem {
    if snippet_support {
        c.insert_text_format = Some(InsertTextFormat::SNIPPET);
        c.insert_text = Some(ins_text.to_string());
    }
    c
}

/// This is different than the properties in resolve_element_scope, because it also include the "out" properties
fn properties_for_changed_callbacks(
    mut node: SyntaxNode,
    document_cache: &mut DocumentCache,
) -> Option<Vec<CompletionItem>> {
    let element = loop {
        if let Some(e) = syntax_nodes::Element::new(node.clone()) {
            break e;
        }
        node = node.parent()?;
    };
    let global_tr = document_cache.global_type_registry();
    let tr = element
        .source_file()
        .and_then(|sf| document_cache.get_document_for_source_file(sf))
        .map(|doc| &doc.local_registry)
        .unwrap_or(&global_tr);
    let element_type = lookup_current_element_type((*element).clone(), tr).unwrap_or_default();
    let result = element_type
        .property_list()
        .into_iter()
        .filter(|(_, ty)| ty.is_property_type())
        .map(|(k, ty)| {
            let k = de_normalize_property_name(&element_type, &k).into_owned();
            let mut c = CompletionItem::new_simple(k, ty.to_string());
            c.kind = Some(CompletionItemKind::PROPERTY);
            c
        })
        .chain(element.PropertyDeclaration().filter_map(|pr| {
            let mut c = CompletionItem::new_simple(
                pr.DeclaredIdentifier().child_text(SyntaxKind::Identifier)?.to_string(),
                pr.Type().map(|t| t.text().into()).unwrap_or_else(|| "property".to_owned()),
            );
            c.kind = Some(CompletionItemKind::PROPERTY);
            Some(c)
        }))
        .chain(i_slint_compiler::typeregister::reserved_properties().filter_map(|(k, ty, _)| {
            if !ty.is_property_type() {
                return None;
            }
            let mut c = CompletionItem::new_simple(k.into(), ty.to_string());
            c.kind = Some(CompletionItemKind::PROPERTY);
            Some(c)
        }));
    Some(result.collect())
}

/// Try to return the completion items for the location inside an element.
/// `FooBar { /* HERE */ }`
/// So properties and potential sub elements
fn resolve_element_scope(
    element: syntax_nodes::Element,
    document_cache: &DocumentCache,
    with_snippets: bool,
) -> Option<Vec<CompletionItem>> {
    let apply_property_ty =
        |mut c: CompletionItem, ty: &Type, cb_args: Option<&[SmolStr]>| -> CompletionItem {
            if matches!(ty, Type::InferredCallback | Type::Callback { .. }) {
                c.kind = Some(CompletionItemKind::METHOD);
                let ins_text = match cb_args {
                    Some(a) => format!("{}({}) => {{$1}}", c.label, a.join(", ")),
                    None => format!("{} => {{$1}}", c.label),
                };
                with_insert_text(c, &ins_text, with_snippets)
            } else {
                c.kind = Some(CompletionItemKind::PROPERTY);
                let ins_text = format!("{}: ", c.label);
                with_insert_text(c, &ins_text, with_snippets)
            }
        };

    let global_tr = document_cache.global_type_registry();
    let tr = element
        .source_file()
        .and_then(|sf| document_cache.get_document_for_source_file(sf))
        .map(|doc| &doc.local_registry)
        .unwrap_or(&global_tr);
    let element_type = lookup_current_element_type((*element).clone(), tr).unwrap_or_default();
    let mut result = element_type
        .property_list()
        .into_iter()
        .filter(|(k, ty)| {
            if matches!(ty, Type::Function { .. }) {
                return false;
            }
            let mut lk = element_type.lookup_property(k);
            lk.is_local_to_component = false;
            lk.is_valid_for_assignment()
        })
        .map(|(k, ty)| {
            let cb_args = if let Type::Callback(f) = &ty {
                if with_snippets
                    && !f.args.is_empty()
                    && f.args.len() == f.arg_names.len()
                    && f.arg_names.iter().all(|x| !x.is_empty())
                {
                    Some(&f.arg_names[..])
                } else {
                    None
                }
            } else {
                None
            };
            let k = de_normalize_property_name(&element_type, &k).into_owned();
            let mut c = CompletionItem::new_simple(k, ty.to_string());
            c.sort_text = Some(format!("#{}", c.label));
            apply_property_ty(c, &ty, cb_args)
        })
        .chain(element.PropertyDeclaration().filter_map(|pr| {
            let name = pr.DeclaredIdentifier().child_text(SyntaxKind::Identifier)?;
            let mut c = CompletionItem::new_simple(
                name.to_string(),
                pr.Type().map(|t| t.text().into()).unwrap_or_else(|| "property".to_owned()),
            );
            c.kind = Some(CompletionItemKind::PROPERTY);
            c.sort_text = Some(format!("#{}", c.label));
            Some(with_insert_text(c, &format!("{name}: "), with_snippets))
        }))
        .chain(element.CallbackDeclaration().filter_map(|cd| {
            let name = cd.DeclaredIdentifier().child_text(SyntaxKind::Identifier)?;
            let mut c = CompletionItem::new_simple(name.to_string(), "callback".into());
            c.kind = Some(CompletionItemKind::METHOD);
            c.sort_text = Some(format!("#{}", c.label));
            Some(with_insert_text(c, &format!("{name} => {{$1}}"), with_snippets))
        }))
        .collect::<Vec<_>>();

    if !matches!(element_type, ElementType::Global) {
        fn accepts_children(
            element_type: &ElementType,
            tr: &TypeRegister,
        ) -> (bool, bool, Vec<SmolStr>) {
            match element_type {
                ElementType::Component(component) => {
                    let base_type = match &*component.child_insertion_point.borrow() {
                        Some(insert_in) => insert_in.0.borrow().base_type.clone(),
                        None => {
                            let base_type = component.root_element.borrow().base_type.clone();
                            if base_type == tr.empty_type() {
                                return (false, true, vec![]);
                            }
                            base_type
                        }
                    };
                    accepts_children(&base_type, tr)
                }
                ElementType::Builtin(builtin_element) => {
                    let mut extra: Vec<SmolStr> =
                        builtin_element.additional_accepted_child_types.keys().cloned().collect();
                    if builtin_element.additional_accept_self {
                        extra.push(builtin_element.name.clone())
                    }
                    (
                        !builtin_element.disallow_global_types_as_child_elements,
                        !builtin_element.is_non_item_type,
                        extra,
                    )
                }
                _ => (true, true, vec![]),
            }
        }
        let (accepts_children, is_item, extra) = accepts_children(&element_type, tr);
        if is_item {
            result.extend(i_slint_compiler::typeregister::reserved_properties().filter_map(
                |(k, ty, _)| {
                    if matches!(ty, Type::Function { .. }) {
                        return None;
                    }
                    let c = CompletionItem::new_simple(k.into(), ty.to_string());
                    Some(apply_property_ty(c, &ty, None))
                },
            ));
        }
        result.extend(extra.into_iter().map(|k| {
            let mut c = CompletionItem::new_simple(k.to_string(), "element".into());
            c.kind = Some(CompletionItemKind::CLASS);
            with_insert_text(c, &format!("{k} {{$1}}"), with_snippets)
        }));
        if accepts_children {
            result.extend(tr.all_elements().into_iter().filter_map(|(k, t)| {
                match t {
                    ElementType::Component(c) if !c.is_global() => (),
                    ElementType::Builtin(b) if !b.is_internal && !b.is_global => (),
                    _ => return None,
                };
                let mut c = CompletionItem::new_simple(k.to_string(), "element".into());
                c.kind = Some(CompletionItemKind::CLASS);
                Some(with_insert_text(c, &format!("{k} {{$1}}"), with_snippets))
            }));
        }
    };
    Some(result)
}

/// Given a property name in the specified element, give the non-normalized name (so that the '_' and '-' fits the definition of the property)
fn de_normalize_property_name<'a>(element_type: &ElementType, prop: &'a str) -> Cow<'a, str> {
    match element_type {
        ElementType::Component(base) => {
            de_normalize_property_name_with_element(&base.root_element, prop)
        }
        _ => prop.into(),
    }
}

// Same as de_normalize_property_name, but use a `ElementRc`
fn de_normalize_property_name_with_element<'a>(element: &ElementRc, prop: &'a str) -> Cow<'a, str> {
    if let Some(d) = element.borrow().property_declarations.get(prop) {
        d.node
            .as_ref()
            .and_then(|n| n.child_node(SyntaxKind::DeclaredIdentifier))
            .and_then(|n| n.child_text(SyntaxKind::Identifier))
            .map_or(prop.into(), |x| x.to_string().into())
    } else {
        de_normalize_property_name(&element.borrow().base_type, prop)
    }
}

fn resolve_expression_scope(
    lookup_context: &LookupCtx,
    document_cache: &common::DocumentCache,
    snippet_support: bool,
) -> Option<Vec<CompletionItem>> {
    let mut r = Vec::new();
    let global = i_slint_compiler::lookup::global_lookup();
    global.for_each_entry(lookup_context, &mut |str, expr| -> Option<()> {
        r.push(completion_item_from_expression(str, expr));
        None
    });
    if snippet_support {
        if let Some(token) = lookup_context.current_token.as_ref().and_then(|t| match t {
            i_slint_compiler::parser::NodeOrToken::Node(n) => n.first_token(),
            i_slint_compiler::parser::NodeOrToken::Token(t) => Some(t.clone()),
        }) {
            let mut available_types: HashSet<String> = r.iter().map(|c| c.label.clone()).collect();
            build_import_statements_edits(
                &token,
                document_cache,
                &mut |ci: &common::ComponentInformation| {
                    if !ci.is_global || !ci.is_exported || available_types.contains(&ci.name) {
                        false
                    } else {
                        available_types.insert(ci.name.clone());
                        true
                    }
                },
                &mut |exported_name, file, the_import| {
                    r.push(CompletionItem {
                        label: format!("{exported_name} (import from \"{file}\")"),
                        insert_text: Some(exported_name.to_string()),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        filter_text: Some(exported_name.to_string()),
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some(format!("(import from \"{file}\")")),
                        additional_text_edits: Some(vec![the_import]),
                        ..Default::default()
                    });
                },
            );
        }
    }
    Some(r)
}

fn completion_item_from_expression(str: &str, lookup_result: LookupResult) -> CompletionItem {
    match lookup_result {
        LookupResult::Expression { expression, .. } => {
            let label = match &expression {
                Expression::PropertyReference(nr) => {
                    de_normalize_property_name_with_element(&nr.element(), str).into_owned()
                }
                Expression::ElementReference(e) => e
                    .upgrade()
                    .and_then(|e| e.borrow().debug.first().and_then(|d| d.node.parent()))
                    .and_then(|n| match n.kind() {
                        SyntaxKind::SubElement => n.child_text(SyntaxKind::Identifier),
                        // global
                        SyntaxKind::Component => n
                            .child_node(SyntaxKind::DeclaredIdentifier)
                            .and_then(|n| n.child_text(SyntaxKind::Identifier)),
                        _ => None,
                    })
                    .filter(|x| i_slint_compiler::parser::normalize_identifier(x) == str)
                    .map_or_else(|| str.to_string(), |t| t.to_string()),
                _ => str.to_string(),
            };

            let mut c = CompletionItem::new_simple(label, expression.ty().to_string());
            c.kind = match expression {
                Expression::BoolLiteral(_) => Some(CompletionItemKind::CONSTANT),
                Expression::PropertyReference(_) => Some(CompletionItemKind::PROPERTY),
                Expression::ElementReference(_) => Some(CompletionItemKind::CLASS),
                Expression::RepeaterIndexReference { .. } => Some(CompletionItemKind::VARIABLE),
                Expression::RepeaterModelReference { .. } => Some(CompletionItemKind::VARIABLE),
                Expression::FunctionParameterReference { .. } => Some(CompletionItemKind::VARIABLE),
                Expression::Cast { .. } => Some(CompletionItemKind::CONSTANT),
                Expression::EasingCurve(_) => Some(CompletionItemKind::CONSTANT),
                Expression::EnumerationValue(_) => Some(CompletionItemKind::ENUM_MEMBER),
                _ => None,
            };
            c
        }
        LookupResult::Enumeration(e) => {
            let mut c = CompletionItem::new_simple(str.to_string(), e.name.to_string());
            c.kind = Some(CompletionItemKind::ENUM);
            c
        }
        LookupResult::Namespace(_) => CompletionItem {
            label: str.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            ..CompletionItem::default()
        },
        LookupResult::Callable(callable) => {
            let label = if let LookupResultCallable::Callable(
                Callable::Callback(nr) | Callable::Function(nr),
            ) = &callable
            {
                de_normalize_property_name_with_element(&nr.element(), str).into_owned()
            } else {
                str.to_string()
            };
            let kind = if matches!(
                callable,
                LookupResultCallable::MemberFunction { .. }
                    | LookupResultCallable::Callable(Callable::Callback(..))
            ) {
                CompletionItemKind::METHOD
            } else {
                CompletionItemKind::FUNCTION
            };
            CompletionItem { label, kind: Some(kind), ..CompletionItem::default() }
        }
    }
}

fn resolve_type_scope(
    token: SyntaxToken,
    document_cache: &DocumentCache,
) -> Option<Vec<CompletionItem>> {
    let global_tr = document_cache.global_type_registry();
    let tr = token
        .source_file()
        .and_then(|sf| document_cache.get_document_for_source_file(sf))
        .map(|doc| &doc.local_registry)
        .unwrap_or(&global_tr);
    Some(
        tr.all_types()
            .into_iter()
            .filter_map(|(k, t)| {
                t.is_property_type().then(|| {
                    let mut c = CompletionItem::new_simple(k.to_string(), String::new());
                    c.kind = Some(CompletionItemKind::TYPE_PARAMETER);
                    c
                })
            })
            .collect(),
    )
}

fn complete_path_in_string(
    base: &Path,
    text: &str,
    offset: TextSize,
) -> Option<Vec<CompletionItem>> {
    let offset = u32::from(offset) as usize;
    if offset > text.len() || offset == 0 || !text.is_char_boundary(offset) {
        return None;
    }
    let mut text = text.strip_prefix('\"')?;
    text = &text[..(offset - 1)];
    let base = i_slint_compiler::typeloader::base_directory(base);
    let path = if let Some(last_slash) = text.rfind('/') {
        base.join(Path::new(&text[..last_slash]))
    } else {
        base
    };
    let dir = std::fs::read_dir(path).ok()?;
    Some(
        dir.filter_map(|x| {
            let entry = x.ok()?;
            let mut c =
                CompletionItem::new_simple(entry.file_name().into_string().ok()?, String::new());
            if entry.file_type().ok()?.is_dir() {
                c.kind = Some(CompletionItemKind::FOLDER);
                c.insert_text = Some(format!("{}/", c.label));
            } else {
                c.kind = Some(CompletionItemKind::FILE);
            }
            Some(c)
        })
        .collect(),
    )
}

/// Add the components that are available when adding import to the `result`
///
/// `available_types`  are the component which are already available and need no
/// import and should already be in result
fn add_components_to_import(
    token: &SyntaxToken,
    document_cache: &common::DocumentCache,
    result: &mut Vec<CompletionItem>,
) {
    let mut available_types: HashSet<_> = result.iter().map(|c| c.label.clone()).collect();
    build_import_statements_edits(
        token,
        document_cache,
        &mut |ci: &common::ComponentInformation| {
            if ci.is_global || !ci.is_exported || available_types.contains(&ci.name) {
                false
            } else {
                available_types.insert(ci.name.clone());
                true
            }
        },
        &mut |exported_name, file, the_import| {
            result.push(CompletionItem {
                label: format!("{exported_name} (import from \"{file}\")"),
                insert_text: if is_followed_by_brace(token) {
                    Some(exported_name.to_string())
                } else {
                    Some(format!("{exported_name} {{$1}}"))
                },
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                filter_text: Some(exported_name.to_string()),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(format!("(import from \"{file}\")")),
                additional_text_edits: Some(vec![the_import]),
                ..Default::default()
            });
        },
    );
}

/// Find the insert location for new imports in the `document`
///
/// The result is a tuple with the first element pointing to the place new import statements should
/// get added. The second element in the tuple is a HashMap mapping import file names to the
/// correct location to enter more components into the existing import statement.
fn find_import_locations(
    document: &syntax_nodes::Document,
) -> (Position, HashMap<String, Position>) {
    let mut import_locations = HashMap::new();
    let mut last = 0u32;
    for import in document.ImportSpecifier() {
        if let Some((loc, file)) = import.ImportIdentifierList().and_then(|list| {
            let node = list.ImportIdentifier().last()?;
            let id = crate::util::last_non_ws_token(&node).or_else(|| node.first_token())?;
            Some((
                text_size_to_lsp_position(id.source_file()?, id.text_range().end()),
                import.child_token(SyntaxKind::StringLiteral)?,
            ))
        }) {
            import_locations.insert(file.text().to_string().trim_matches('\"').to_string(), loc);
        }
        last = import.text_range().end().into();
    }

    let new_import_position = if last == 0 {
        // There are currently no input statement, place it at the location of the first non-empty token.
        // This should also work in the slint! macro.
        // consider this file:  We want to insert before the doc1 position
        // ```
        // //not doc (eg, license header)
        //
        // //doc1
        // //doc2
        // component Foo {
        // ```
        let mut offset = None;
        for it in document.children_with_tokens() {
            match it.kind() {
                SyntaxKind::Comment => {
                    if offset.is_none() {
                        offset = Some(it.text_range().start());
                    }
                }
                SyntaxKind::Whitespace => {
                    // Single newline is just considered part of the comment
                    // but more new lines means it splits that comment
                    if it.as_token().unwrap().text() != "\n" {
                        offset = None;
                    }
                }
                _ => {
                    if offset.is_none() {
                        offset = Some(it.text_range().start());
                    }
                    break;
                }
            }
        }
        text_size_to_lsp_position(&document.source_file, offset.unwrap_or_default())
    } else {
        Position::new(text_size_to_lsp_position(&document.source_file, last.into()).line + 1, 0)
    };

    (new_import_position, import_locations)
}

fn create_import_edit_impl(
    component: &str,
    import_path: &str,
    missing_import_location: &Position,
    known_import_locations: &HashMap<String, Position>,
) -> TextEdit {
    known_import_locations.get(import_path).map_or_else(
        || {
            TextEdit::new(
                Range::new(*missing_import_location, *missing_import_location),
                format!("import {{ {component} }} from \"{import_path}\";\n"),
            )
        },
        |pos| TextEdit::new(Range::new(*pos, *pos), format!(", {component}")),
    )
}

/// Creates a text edit
#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub fn create_import_edit(
    document: &i_slint_compiler::object_tree::Document,
    component: &str,
    import_path: &Option<String>,
) -> Option<TextEdit> {
    let import_path = import_path.as_ref()?;
    let doc_node = document.node.as_ref().unwrap();

    if document.local_registry.lookup_element(component).is_ok() {
        None // already known, no import needed
    } else {
        let (missing_import_location, known_import_locations) = find_import_locations(doc_node);

        Some(create_import_edit_impl(
            component,
            import_path,
            &missing_import_location,
            &known_import_locations,
        ))
    }
}

/// Try to generate `import { XXX } from "foo.slint";` for every component
///
/// This is used for auto-completion and also for fixup diagnostics
///
/// Call `add_edit` with the component name and file name and TextEdit for every component for which the `filter` callback returns true
pub fn build_import_statements_edits(
    token: &SyntaxToken,
    document_cache: &common::DocumentCache,
    filter: &mut dyn FnMut(&common::ComponentInformation) -> bool,
    add_edit: &mut dyn FnMut(&str, &str, TextEdit),
) -> Option<()> {
    // Find out types that can be imported
    let current_file = token.source_file.path().to_owned();
    let current_uri = lsp_types::Url::from_file_path(&current_file).ok();
    let current_doc =
        document_cache.get_document_for_source_file(&token.source_file)?.node.as_ref()?;
    let (missing_import_location, known_import_locations) = find_import_locations(current_doc);

    let exports = {
        let mut tmp = Vec::new();
        all_exported_components(document_cache, filter, &mut tmp);
        tmp
    };

    for ci in &exports {
        let Some(file) = ci.import_file_name(&current_uri) else {
            continue;
        };

        let the_import = create_import_edit_impl(
            &ci.name,
            &file,
            &missing_import_location,
            &known_import_locations,
        );
        add_edit(&ci.name, &file, the_import);
    }

    Some(())
}

fn is_followed_by_brace(token: &SyntaxToken) -> bool {
    let mut next_token = token.next_token();
    while let Some(ref t) = next_token {
        if t.kind() != SyntaxKind::Whitespace {
            break;
        }
        next_token = t.next_token();
    }
    next_token.is_some_and(|x| x.kind() == SyntaxKind::LBrace)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Given a source text containing the unicode emoji `🔺`, the emoji will be removed and then an autocompletion request will be done as if the cursor was there
    fn get_completions(file: &str) -> Option<Vec<CompletionItem>> {
        const CURSOR_EMOJI: char = '🔺';
        let offset = (file.find(CURSOR_EMOJI).unwrap() as u32).into();
        let source = file.replace(CURSOR_EMOJI, "");
        let (mut dc, uri, _) = crate::language::test::loaded_document_cache(source);

        let doc = dc.get_document(&uri).unwrap();
        let token = crate::language::token_at_offset(doc.node.as_ref().unwrap(), offset)?;
        let caps = CompletionClientCapabilities {
            completion_item: Some(lsp_types::CompletionItemCapability {
                snippet_support: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };

        completion_at(&mut dc, token, offset, Some(&caps))
    }

    #[test]
    fn in_expression() {
        let with_semi = r#"
            component Bar inherits Text { nope := Rectangle {} property <string> red; }
            global The_Glib { property <int> gama; }
            component Foo {
                property <int> alpha;
                pure function funi() {}
                the_bo-bo := Bar {
                    property <int> beta;
                    width: 🔺;
                }
            }
        "#;
        let without_semi = r#"
            component Bar inherits Text { nope := Rectangle {} property <string> red; }
            global The_Glib { property <int> gama; }
            component Foo {
                property <int> alpha;
                pure function funi() {}
                the_bo-bo := Bar {
                    property <int> beta;
                    width: 🔺
                }
            }
        "#;
        for source in [with_semi, without_semi] {
            let res = get_completions(source).unwrap();
            res.iter().find(|ci| ci.label == "alpha").unwrap();
            res.iter().find(|ci| ci.label == "beta").unwrap();
            res.iter().find(|ci| ci.label == "funi").unwrap();
            res.iter().find(|ci| ci.label == "The_Glib").unwrap();
            res.iter().find(|ci| ci.label == "Colors").unwrap();
            res.iter().find(|ci| ci.label == "Math").unwrap();
            res.iter().find(|ci| ci.label == "animation-tick").unwrap();
            res.iter().find(|ci| ci.label == "the_bo-bo").unwrap();
            res.iter().find(|ci| ci.label == "true").unwrap();
            res.iter().find(|ci| ci.label == "self").unwrap();
            res.iter().find(|ci| ci.label == "root").unwrap();
            res.iter().find(|ci| ci.label == "TextInputInterface").unwrap();
            let palette = res
                .iter()
                .find(|ci| ci.insert_text.as_ref().is_some_and(|t| t == "Palette"))
                .unwrap();
            assert_eq!(
                palette.additional_text_edits.as_ref().unwrap()[0].new_text,
                "import { Palette } from \"std-widgets.slint\";\n"
            );

            assert!(!res.iter().any(|ci| ci.label == "text"));
            assert!(!res.iter().any(|ci| ci.label == "red"));
            assert!(!res.iter().any(|ci| ci.label == "nope"));

            assert!(!res.iter().any(|ci| ci.label == "Rectangle"));
            assert!(!res.iter().any(|ci| ci.label == "Clip"));
            assert!(!res.iter().any(|ci| ci.label == "NativeStyleMetrics"));
            assert!(!res.iter().any(|ci| ci.label == "SlintInternal"));
        }
    }

    #[test]
    fn in_element() {
        let source = r#"
            component Bar inherits TouchArea {
                callback the-callback();
                private property <int> priv_prop;
                in property <[string]> in_prop;
                out property <string> out_prop;
                property <int> priv_prop2;
                function priv_func() {}
                public pure function pub_func() {}
                @children
            }
            component Foo {
                Bar {
                    🔺

                    property <int> local-prop;
                }
            }
        "#;
        let res = get_completions(source).unwrap();

        const P: Option<CompletionItemKind> = Some(CompletionItemKind::PROPERTY);
        const M: Option<CompletionItemKind> = Some(CompletionItemKind::METHOD);

        // from TouchArea
        assert_eq!(res.iter().find(|ci| ci.label == "enabled").unwrap().kind, P);
        assert_eq!(res.iter().find(|ci| ci.label == "clicked").unwrap().kind, M);
        // general
        assert_eq!(res.iter().find(|ci| ci.label == "width").unwrap().kind, P);
        assert_eq!(res.iter().find(|ci| ci.label == "y").unwrap().kind, P);
        assert_eq!(res.iter().find(|ci| ci.label == "accessible-role").unwrap().kind, P);
        assert_eq!(res.iter().find(|ci| ci.label == "opacity").unwrap().kind, P);
        // from Bar
        assert_eq!(res.iter().find(|ci| ci.label == "in_prop").unwrap().kind, P);
        assert_eq!(res.iter().find(|ci| ci.label == "the-callback").unwrap().kind, M);
        // local
        assert_eq!(res.iter().find(|ci| ci.label == "local-prop").unwrap().kind, P);
        // no functions, no private stuff
        assert_eq!(res.iter().find(|ci| ci.label == "out_prop" || ci.label == "out-prop"), None);
        assert_eq!(res.iter().find(|ci| ci.label == "priv_prop" || ci.label == "priv-prop"), None);
        assert_eq!(
            res.iter().find(|ci| ci.label == "priv_prop1" || ci.label == "priv-prop1"),
            None
        );
        assert_eq!(res.iter().find(|ci| ci.label == "priv_func" || ci.label == "priv-func"), None);
        assert_eq!(res.iter().find(|ci| ci.label == "pub_func" || ci.label == "pub-func"), None);
        assert_eq!(res.iter().find(|ci| ci.label == "pressed"), None);
        assert_eq!(res.iter().find(|ci| ci.label == "pressed-x"), None);

        // elements
        let class = Some(CompletionItemKind::CLASS);
        assert_eq!(res.iter().find(|ci| ci.label == "Bar").unwrap().kind, class);
        assert_eq!(res.iter().find(|ci| ci.label == "Text").unwrap().kind, class);
        assert_eq!(res.iter().find(|ci| ci.label == "Rectangle").unwrap().kind, class);
        assert_eq!(res.iter().find(|ci| ci.label == "TouchArea").unwrap().kind, class);
        assert_eq!(res.iter().find(|ci| ci.label == "VerticalLayout").unwrap().kind, class);
        assert!(!res.iter().any(|ci| ci.label == "MenuItem"));

        // keywords
        assert_eq!(
            res.iter().find(|ci| ci.label == "changed").unwrap().kind,
            Some(CompletionItemKind::KEYWORD)
        );
        assert_eq!(
            res.iter().find(|ci| ci.label == "animate").unwrap().kind,
            Some(CompletionItemKind::KEYWORD)
        );
        assert_eq!(
            res.iter().find(|ci| ci.label == "states").unwrap().kind,
            Some(CompletionItemKind::KEYWORD)
        );
        assert_eq!(
            res.iter().find(|ci| ci.label == "for").unwrap().kind,
            Some(CompletionItemKind::KEYWORD)
        );
        assert_eq!(
            res.iter().find(|ci| ci.label == "if").unwrap().kind,
            Some(CompletionItemKind::KEYWORD)
        );

        // check snippets:
        assert_eq!(
            res.iter().find(|ci| ci.label == "local-prop").unwrap().insert_text,
            Some("local-prop: ".into())
        );
        assert_eq!(
            res.iter().find(|ci| ci.label == "the-callback").unwrap().insert_text,
            Some("the-callback => {$1}".into())
        );
    }

    #[test]
    fn in_element_builtin() {
        let source = r#"
            component Foo {
                FocusScope {
                    function func() {}
                    property <int> local-prop;
                    🔺
                }
            }
        "#;
        let res = get_completions(source).unwrap();
        // from FocusScope
        res.iter().find(|ci| ci.label == "enabled").unwrap();
        res.iter().find(|ci| ci.label == "key-pressed").unwrap();
        // general
        res.iter().find(|ci| ci.label == "width").unwrap();
        res.iter().find(|ci| ci.label == "y").unwrap();
        res.iter().find(|ci| ci.label == "accessible-role").unwrap();
        res.iter().find(|ci| ci.label == "opacity").unwrap();
        // local
        res.iter().find(|ci| ci.label == "local-prop").unwrap();
        // no functions, no private stuff
        assert_eq!(res.iter().find(|ci| ci.label == "has-focus"), None);
        assert_eq!(res.iter().find(|ci| ci.label == "func"), None);

        // elements
        let class = Some(CompletionItemKind::CLASS);
        assert_eq!(res.iter().find(|ci| ci.label == "Flickable").unwrap().kind, class);
        assert_eq!(res.iter().find(|ci| ci.label == "Rectangle").unwrap().kind, class);
        assert_eq!(res.iter().find(|ci| ci.label == "HorizontalLayout").unwrap().kind, class);
        assert!(!res.iter().any(|ci| ci.label == "MenuItem"));
    }

    #[test]
    fn in_menu_item() {
        let source = r#"
            component Foo inherits Window {
                property <int> local-prop;
                MenuBar {
                    Menu {
                        property <int> local-prop2;
                        🔺
                    }
                }
            }
        "#;
        let res = get_completions(source).unwrap();

        const P: Option<CompletionItemKind> = Some(CompletionItemKind::PROPERTY);

        // general
        assert!(!res.iter().any(|ci| ci.label == "width"));
        assert!(!res.iter().any(|ci| ci.label == "y"));
        assert!(!res.iter().any(|ci| ci.label == "accessible-role"));
        assert!(!res.iter().any(|ci| ci.label == "opacity"));

        // elements
        assert!(!res.iter().any(|ci| ci.label == "Rectangle"));
        let class = Some(CompletionItemKind::CLASS);
        assert_eq!(res.iter().find(|ci| ci.label == "MenuItem").unwrap().kind, class);

        // properties
        assert_eq!(res.iter().find(|ci| ci.label == "title").unwrap().kind, P);
        assert!(!res.iter().any(|ci| ci.label == "activated"));

        // local
        assert!(!res.iter().any(|ci| ci.label == "local-prop"));
        assert_eq!(res.iter().find(|ci| ci.label == "local-prop2").unwrap().kind, P);

        // keywords
        assert_eq!(
            res.iter().find(|ci| ci.label == "for").unwrap().kind,
            Some(CompletionItemKind::KEYWORD)
        );
        assert_eq!(
            res.iter().find(|ci| ci.label == "if").unwrap().kind,
            Some(CompletionItemKind::KEYWORD)
        );
    }

    #[test]
    fn dashes_and_underscores() {
        let in_element = r#"
            component Bar { in property <string> super_property-1; }
            component Foo {
                Bar {
                    function nope() {}
                    property<int> hello_world;
                    pure callback with_underscores-and_dash();
                    🔺
                }
            }
        "#;
        let in_expr1 = r#"
        component Bar { property <string> nope; }
        component Foo {
            function hello_world() {}
            Bar {
                in property <string> super_property-1;
                pure callback with_underscores-and_dash();
                width: 🔺
            }
        }
        "#;
        let in_expr2 = r#"
        component Bar { in property <string> super_property-1; property <string> nope; }
        component Foo {
            property <int> nope;
            Bar {
                function hello_world() {}
                pure callback with_underscores-and_dash();
                width: self.🔺
            }
        }
        "#;
        for source in [in_element, in_expr1, in_expr2] {
            let res = get_completions(source).unwrap();
            assert!(!res.iter().any(|ci| ci.label == "nope"));
            res.iter().find(|ci| ci.label == "with_underscores-and_dash").unwrap();
            res.iter().find(|ci| ci.label == "super_property-1").unwrap();
            res.iter().find(|ci| ci.label == "hello_world").unwrap();
        }
    }

    #[test]
    fn arguments_struct() {
        let source = r#"
            struct S1 { foo: int, bar: {xx: int, yy: string} }
            component Bar { callback c(S1) }
            component Foo {
                Bar {
                    c(param) => { param.bar.🔺 }
                }
            }
        "#;
        let res = get_completions(source).unwrap();
        res.iter().find(|ci| ci.label == "xx").unwrap();
        res.iter().find(|ci| ci.label == "yy").unwrap();
        assert_eq!(res.len(), 2);
    }

    #[test]
    fn function_args() {
        let source = r#"
            component Foo {
                function xxx(alpha: int, beta_gamma: string) -> color {
                    🔺
                }
            }
        "#;
        let res = get_completions(source).unwrap();
        res.iter().find(|ci| ci.label == "alpha").unwrap();
        res.iter().find(|ci| ci.label == "beta-gamma").unwrap();
        res.iter().find(|ci| ci.label == "red").unwrap();
        assert!(!res.iter().any(|ci| ci.label == "width"));
    }

    #[test]
    fn function_no_when_in_empty_state() {
        let source = r#"
            component Foo {
                states [
                    🔺
                ]
            }
        "#;
        assert!(get_completions(source).is_none());
    }

    #[test]
    fn function_no_when_in_state() {
        let source = r#"
            component Foo {
                property<bool> bar: false;
                states [
                    foo when root.bar: { }
                    🔺
                    baz when !root.bar: { }
                ]
            }
        "#;
        assert!(get_completions(source).is_none());
    }

    #[test]
    fn function_when_after_state_name() {
        let source = r#"
            component Foo {
                states [
                    foo 🔺
                ]
            }
        "#;
        let res = get_completions(source).unwrap();
        res.iter().find(|ci| ci.label == "when").unwrap();
    }

    #[test]
    fn function_when_after_state_name_between_more_states() {
        let source = r#"
            component Foo {
                states [
                    foo when root.bar: { }
                    barbar 🔺
                    baz when !root.bar: { }
                ]
            }
        "#;
        let res = get_completions(source).unwrap();
        res.iter().find(|ci| ci.label == "when").unwrap();
    }

    #[test]
    fn import_component() {
        let source = r#"
            import {🔺} from "std-widgets.slint"
        "#;
        let res = get_completions(source).unwrap();
        res.iter().find(|ci| ci.label == "LineEdit").unwrap();
        res.iter().find(|ci| ci.label == "Palette").unwrap();

        let source = r#"
            import { Foo, 🔺} from "std-widgets.slint"
        "#;
        let res = get_completions(source).unwrap();
        res.iter().find(|ci| ci.label == "TextEdit").unwrap();
    }

    #[test]
    fn animation_completion() {
        let source = r#"
            component Foo {
                Text {
                    width: 20px;
                    animate width {
                        🔺
                    }
                }
            }
        "#;
        let res = get_completions(source).unwrap();
        res.iter().find(|ci| ci.label == "delay").unwrap();
        res.iter().find(|ci| ci.label == "duration").unwrap();
        res.iter().find(|ci| ci.label == "direction").unwrap();
        res.iter().find(|ci| ci.label == "iteration-count").unwrap();
        res.iter().find(|ci| ci.label == "easing").unwrap();
    }

    #[test]
    fn animation_easing_completion() {
        let source = r#"
            component Foo {
                Text {
                    width: 20px;
                    animate width {
                        easing: 🔺;
                    }
                }
            }
        "#;
        let res = get_completions(source).unwrap();
        res.iter().find(|ci| ci.label == "ease-in-quad").unwrap();
        res.iter().find(|ci| ci.label == "ease-out-quad").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-out-quad").unwrap();
        res.iter().find(|ci| ci.label == "ease").unwrap();
        res.iter().find(|ci| ci.label == "ease-in").unwrap();
        res.iter().find(|ci| ci.label == "ease-out").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-out").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-quart").unwrap();
        res.iter().find(|ci| ci.label == "ease-out-quart").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-out-quart").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-quint").unwrap();
        res.iter().find(|ci| ci.label == "ease-out-quint").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-out-quint").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-expo").unwrap();
        res.iter().find(|ci| ci.label == "ease-out-expo").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-out-expo").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-sine").unwrap();
        res.iter().find(|ci| ci.label == "ease-out-sine").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-out-sine").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-back").unwrap();
        res.iter().find(|ci| ci.label == "ease-out-back").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-out-back").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-elastic").unwrap();
        res.iter().find(|ci| ci.label == "ease-out-elastic").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-out-elastic").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-bounce").unwrap();
        res.iter().find(|ci| ci.label == "ease-out-bounce").unwrap();
        res.iter().find(|ci| ci.label == "ease-in-out-bounce").unwrap();
        res.iter().find(|ci| ci.label == "linear").unwrap();
        res.iter().find(|ci| ci.label == "cubic-bezier").unwrap();
    }

    #[test]
    fn changed_completion() {
        let source1 = "{ property<int> xyz; changed 🔺 => {} }  ";
        let source2 = "{ property<int> xyz; changed 🔺 }  ";
        let source3 = "{ property<int> xyz; changed t🔺 }  ";
        let source4 = "{ property<int> xyz; changed t🔺 => {} }  ";
        let source5 = "{ property<int> xyz; changed 🔺 \n enabled: true; }  ";
        let source6 = "{ property<int> xyz; changed t🔺 \n enabled: true; }  ";
        let source7 = "{ changed t🔺 => {} property<int> xyz; ";
        for s in [source1, source2, source3, source4, source5, source6, source7] {
            eprintln!("changed_completion: {s:?}");
            let s = format!("component Bar inherits TextInput {{ property <int> nope; out property <int> from_bar; }} component Foo {{ Bar {s} }}");
            let res = get_completions(&s).unwrap();
            res.iter().find(|ci| ci.label == "text").unwrap();
            res.iter().find(|ci| ci.label == "has-focus").unwrap();
            res.iter().find(|ci| ci.label == "width").unwrap();
            res.iter().find(|ci| ci.label == "y").unwrap();
            res.iter().find(|ci| ci.label == "xyz").unwrap();
            res.iter().find(|ci| ci.label == "from_bar").unwrap();

            assert!(!res.iter().any(|ci| ci.label == "Text"));
            assert!(!res.iter().any(|ci| ci.label == "edited"));
            assert!(!res.iter().any(|ci| ci.label == "focus"));
            assert!(!res.iter().any(|ci| ci.label == "nope"));
        }
    }

    #[test]
    fn element_snippet_without_braces() {
        let source = r#"
            component Foo {
                🔺
            }
        "#;
        let res = get_completions(source)
            .unwrap()
            .into_iter()
            .filter(|ci| {
                matches!(
                    ci,
                    CompletionItem {
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        detail: Some(detail),
                        ..
                    }
                    if detail == "element"
                )
            })
            .collect::<Vec<_>>();
        assert!(!res.is_empty());
        assert!(res.iter().all(|ci| ci.insert_text.as_ref().is_some_and(|t| t.ends_with("{$1}"))));
    }

    #[test]
    fn element_snippet_before_braces() {
        let source = r#"
            component Foo {
                🔺 {}
            }
        "#;
        let res = get_completions(source)
            .unwrap()
            .into_iter()
            .filter(|ci| {
                matches!(
                    ci,
                    CompletionItem {
                        detail: Some(detail),
                        ..
                    }
                    if detail == "element"
                )
            })
            .collect::<Vec<_>>();
        assert!(!res.is_empty());
        assert!(res.iter().all(|ci| ci.insert_text.is_none()));
    }

    #[test]
    fn import_completed_component() {
        let source = r#"
            import { VerticalBox                 } from "std-widgets.slint";

            export component Test {
                VerticalBox {
                    🔺
                }
            }

        "#;
        let res = get_completions(source).unwrap();
        let about = res.iter().find(|ci| ci.label.starts_with("AboutSlint")).unwrap();

        let additional_edits = about.additional_text_edits.as_ref().unwrap();
        let edit = additional_edits.first().unwrap();

        assert_eq!(edit.range.start.line, 1);
        assert_eq!(edit.range.start.character, 32);
        assert_eq!(edit.range.end.line, 1);
        assert_eq!(edit.range.end.character, 32);
        assert_eq!(edit.new_text, ", AboutSlint");
    }

    #[test]
    fn inherits() {
        let sources = [
            "component Bar 🔺",
            "component Bar in🔺",
            "component Bar 🔺 {}",
            "component Bar in🔺 Window {}",
        ];
        for source in sources {
            eprintln!("Test for inherits in {source:?}");
            let res = get_completions(source).unwrap();
            res.iter().find(|ci| ci.label == "inherits").unwrap();
        }

        let sources = ["component 🔺", "component Bar {}🔺", "component Bar inherits 🔺 {}", "🔺"];
        for source in sources {
            let Some(res) = get_completions(source) else { continue };
            assert!(
                !res.iter().any(|ci| ci.label == "inherits"),
                "completion for {source:?} contains 'inherits'"
            );
        }
    }

    #[test]
    fn two_way_bindings() {
        let sources = [
            "component X { property<string> prop; elem := Text{} property foo <=> 🔺",
            "component X { property<string> prop; elem := Text{} property<string> foo <=> e🔺; }",
            "component X { property<string> prop; elem := Text{} prop <=> 🔺",
            "component X { property<string> prop; elem := Text{} prop <=> e🔺; }",
        ];
        for source in sources {
            eprintln!("Test for two ways in {source:?}");
            let res = get_completions(source).unwrap();
            res.iter().find(|ci| ci.label == "prop").unwrap();
            res.iter().find(|ci| ci.label == "self").unwrap();
            res.iter().find(|ci| ci.label == "root").unwrap();
            res.iter().find(|ci| ci.label == "elem").unwrap();
        }

        let sources = [
            "component X { elem := Text{ property<int> prop; } property foo <=> elem.🔺",
            "component X { elem := Text{ property<int> prop; } property <string> foo <=> elem.t🔺",
            "component X { elem := Text{ property<int> prop; } property foo <=> elem.🔺; }",
            "component X { elem := Text{ property<string> prop; } title <=> elem.t🔺",
            "component X { elem := Text{ property<string> prop; } title <=> elem.🔺; }",
        ];
        for source in sources {
            eprintln!("Test for two ways in {source:?}");
            let res = get_completions(source).unwrap();
            res.iter().find(|ci| ci.label == "text").unwrap();
            res.iter().find(|ci| ci.label == "prop").unwrap();
            assert!(!res.iter().any(|ci| ci.label == "elem"));
        }
    }

    #[test]
    fn callback_args() {
        let source = r#"
            import { StandardTableView } from "std-widgets.slint";
            component Foo {
                StandardTableView {
                    🔺
                }
            }
        "#;
        let res = get_completions(source).unwrap();
        assert_eq!(
            res.iter().find(|ci| ci.label == "row-pointer-event").unwrap().insert_text,
            Some("row-pointer-event(row, event, position) => {$1}".into())
        );
        // builtin callback don't have named argument yet
        assert_eq!(
            res.iter().find(|ci| ci.label == "accessible-action-set-value").unwrap().insert_text,
            Some("accessible-action-set-value => {$1}".into())
        );
        let source = r#"
            import { StandardTableView } from "std-widgets.slint";
            component MyTouchArea inherits TouchArea {
                callback cb1;
                callback cb2(foo: int, bar_bar: string);
                callback cb3(int, xx: string); // only one arg is named
            }
            component Foo {
                MyTouchArea {
                    🔺
                }
            }
        "#;
        let res = get_completions(source).unwrap();
        assert_eq!(
            res.iter().find(|ci| ci.label == "pointer-event").unwrap().insert_text,
            Some("pointer-event(event) => {$1}".into())
        );
        assert_eq!(
            res.iter().find(|ci| ci.label == "clicked").unwrap().insert_text,
            Some("clicked => {$1}".into())
        );
        assert_eq!(
            res.iter().find(|ci| ci.label == "cb1").unwrap().insert_text,
            Some("cb1 => {$1}".into())
        );
        assert_eq!(
            res.iter().find(|ci| ci.label == "cb2").unwrap().insert_text,
            Some("cb2(foo, bar-bar) => {$1}".into())
        );
        assert_eq!(
            res.iter().find(|ci| ci.label == "cb3").unwrap().insert_text,
            Some("cb3 => {$1}".into())
        );
    }
}
