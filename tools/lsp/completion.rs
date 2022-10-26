// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use super::util::lookup_current_element_type;
use super::DocumentCache;
#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;
use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::lookup::{LookupCtx, LookupObject, LookupResult};
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxToken};
use lsp_types::{
    CompletionClientCapabilities, CompletionItem, CompletionItemKind, CompletionResponse,
    InsertTextFormat, Position, Range, TextEdit,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(crate) fn completion_at(
    document_cache: &mut DocumentCache,
    token: SyntaxToken,
    offset: u32,
    client_caps: Option<&CompletionClientCapabilities>,
) -> Option<CompletionResponse> {
    let node = token.parent();

    if token.kind() == SyntaxKind::StringLiteral {
        if matches!(node.kind(), SyntaxKind::ImportSpecifier | SyntaxKind::AtImageUrl) {
            return complete_path_in_string(
                token.source_file()?.path(),
                token.text(),
                offset.checked_sub(token.text_range().start().into())?,
            )
            .map(Into::into);
        }
    } else if let Some(element) = syntax_nodes::Element::new(node.clone()) {
        if token.kind() == SyntaxKind::At
            || (token.kind() == SyntaxKind::Identifier
                && token.prev_token().map_or(false, |t| t.kind() == SyntaxKind::At))
        {
            return Some(vec![CompletionItem::new_simple("children".into(), String::new())].into());
        }

        return resolve_element_scope(element, document_cache).map(|mut r| {
            let mut available_types = HashSet::new();
            let snippet_support = client_caps
                .and_then(|caps| caps.completion_item.as_ref())
                .and_then(|caps| caps.snippet_support)
                .unwrap_or(false);
            if snippet_support {
                for c in r.iter_mut() {
                    c.insert_text_format = Some(InsertTextFormat::SNIPPET);
                    match c.kind {
                        Some(CompletionItemKind::PROPERTY) => {
                            c.insert_text = Some(format!("{}: $1;", c.label))
                        }
                        Some(CompletionItemKind::METHOD) => {
                            c.insert_text = Some(format!("{} => {{ $1 }}", c.label))
                        }
                        Some(CompletionItemKind::CLASS) => {
                            available_types.insert(c.label.clone());
                            c.insert_text = Some(format!("{} {{ $1 }}", c.label))
                        }
                        _ => (),
                    }
                }
            }

            // add keywords
            r.extend(
                [
                    ("property", "property <$1> $2;"),
                    ("callback", "callback $1();"),
                    ("animate", "animate $1 { $2 }"),
                    ("states", "states [ $1 ]"),
                    ("transitions", "transitions [ $1 ]"),
                    ("for", "for $1 in $2: $3 {}"),
                    ("if", "if ($1) : $2 {}"),
                    ("@children", "@children"),
                ]
                .iter()
                .map(|(kw, ins_tex)| {
                    let mut c = CompletionItem::new_simple(kw.to_string(), String::new());
                    c.kind = Some(CompletionItemKind::KEYWORD);
                    with_insert_text(c, ins_tex, client_caps)
                }),
            );

            // Find out types that can be imported
            let import_locations = (|| {
                if !snippet_support {
                    return None;
                };
                let current_file = token.source_file.path().to_owned();
                let current_doc =
                    document_cache.documents.get_document(&current_file)?.node.as_ref()?;
                let current_uri = lsp_types::Url::from_file_path(&current_file).ok()?;
                let mut import_locations = HashMap::new();
                let mut last = 0u32;
                for import in current_doc.ImportSpecifier() {
                    if let Some((loc, file)) = import.ImportIdentifierList().and_then(|list| {
                        Some((
                            document_cache.byte_offset_to_position(
                                list.ImportIdentifier().last()?.text_range().end().into(),
                                &current_uri,
                            )?,
                            import.child_token(SyntaxKind::StringLiteral)?,
                        ))
                    }) {
                        import_locations
                            .insert(file.text().to_string().trim_matches('\"').to_string(), loc);
                    }
                    last = import.text_range().end().into();
                }
                let last = if last == 0 {
                    0
                } else {
                    document_cache
                        .byte_offset_to_position(last, &current_uri)
                        .map_or(0, |p| p.line + 1)
                };
                Some((import_locations, last, current_uri))
            })();

            if let Some((import_locations, last, current_uri)) = import_locations {
                for file in document_cache.documents.all_files() {
                    let doc = document_cache.documents.get_document(file).unwrap();
                    let file = if file.starts_with("builtin:/") {
                        match file.file_name() {
                            Some(file) if file == "std-widgets.slint" => "std-widgets.slint".into(),
                            _ => continue,
                        }
                    } else {
                        match lsp_types::Url::make_relative(
                            &current_uri,
                            &lsp_types::Url::from_file_path(file).unwrap(),
                        ) {
                            Some(file) => file,
                            None => continue,
                        }
                    };

                    for (exported_name, ty) in &doc.exports.0 {
                        if available_types.contains(&exported_name.name) {
                            continue;
                        }
                        if let Some(c) = ty.as_ref().left() {
                            if c.is_global() {
                                continue;
                            }
                        } else {
                            continue;
                        }
                        available_types.insert(exported_name.name.clone());
                        let the_import = import_locations.get(&file).map_or_else(
                            || {
                                let pos = Position::new(last, 0);
                                TextEdit::new(
                                    Range::new(pos, pos),
                                    format!(
                                        "import {{ {} }} from \"{}\";\n",
                                        exported_name.name, file
                                    ),
                                )
                            },
                            |pos| {
                                TextEdit::new(
                                    Range::new(*pos, *pos),
                                    format!(", {}", exported_name.name),
                                )
                            },
                        );
                        r.push(CompletionItem {
                            label: format!(
                                "{} (import from from \"{}\")",
                                exported_name.name, file
                            ),
                            insert_text: Some(format!("{} {{ $1 }}", exported_name.name)),
                            insert_text_format: Some(InsertTextFormat::SNIPPET),
                            filter_text: Some(exported_name.name.clone()),
                            kind: Some(CompletionItemKind::CLASS),
                            detail: Some(format!("(import from \"{}\")", file)),
                            additional_text_edits: Some(vec![the_import.into()]),
                            ..Default::default()
                        });
                    }
                }
            }

            r.into()
        });
    } else if let Some(n) = syntax_nodes::Binding::new(node.clone()) {
        if token.kind() != SyntaxKind::Identifier {
            return None;
        }
        let all = resolve_element_scope(syntax_nodes::Element::new(n.parent()?)?, document_cache)?;
        return Some(
            all.into_iter()
                .filter(|ce| ce.kind == Some(CompletionItemKind::PROPERTY))
                .collect::<Vec<_>>()
                .into(),
        );
    } else if let Some(n) = syntax_nodes::TwoWayBinding::new(node.clone()) {
        if token.kind() != SyntaxKind::Identifier {
            return None;
        }
        let all = resolve_element_scope(syntax_nodes::Element::new(n.parent()?)?, document_cache)?;
        return Some(
            all.into_iter()
                .filter(|ce| ce.kind == Some(CompletionItemKind::PROPERTY))
                .collect::<Vec<_>>()
                .into(),
        );
    } else if let Some(n) = syntax_nodes::CallbackConnection::new(node.clone()) {
        if token.kind() != SyntaxKind::Identifier {
            return None;
        }
        let all = resolve_element_scope(syntax_nodes::Element::new(n.parent()?)?, document_cache)?;
        return Some(
            all.into_iter()
                .filter(|ce| ce.kind == Some(CompletionItemKind::METHOD))
                .collect::<Vec<_>>()
                .into(),
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
                && token.prev_token().map_or(false, |t| t.kind() == SyntaxKind::At))
        {
            return Some(
                [
                    ("image-url", "image-url(\"$1\")"),
                    ("linear-gradient", "linear-gradient($1)"),
                    ("radial-gradient", "radial-gradient(circle, $1)"),
                ]
                .into_iter()
                .map(|(label, insert)| {
                    with_insert_text(
                        CompletionItem::new_simple(label.into(), String::new()),
                        insert,
                        client_caps,
                    )
                })
                .collect::<Vec<_>>()
                .into(),
            );
        }

        return crate::util::with_lookup_ctx(document_cache, node, |ctx| {
            resolve_expression_scope(ctx).map(Into::into)
        })?;
    } else if let Some(q) = syntax_nodes::QualifiedName::new(node.clone()) {
        match q.parent()?.kind() {
            SyntaxKind::Element => {
                // auto-complete the types
                let global_tr = document_cache.documents.global_type_registry.borrow();
                let tr = q
                    .source_file()
                    .and_then(|sf| document_cache.documents.get_document(sf.path()))
                    .map(|doc| &doc.local_registry)
                    .unwrap_or(&global_tr);
                return Some(
                    tr.all_elements()
                        .into_iter()
                        .filter_map(|(k, t)| {
                            match t {
                                ElementType::Component(c) if !c.is_global() => (),
                                ElementType::Builtin(b) if !b.is_internal && !b.is_global => (),
                                _ => return None,
                            };
                            let mut c = CompletionItem::new_simple(k, "element".into());
                            c.kind = Some(CompletionItemKind::CLASS);
                            Some(c)
                        })
                        .collect::<Vec<_>>()
                        .into(),
                );
            }
            SyntaxKind::Type => {
                return resolve_type_scope(token, document_cache).map(Into::into);
            }
            SyntaxKind::Expression => {
                return crate::util::with_lookup_ctx(document_cache, node, |ctx| {
                    let it = q.children_with_tokens().filter_map(|t| t.into_token());
                    let mut it = it.skip_while(|t| {
                        t.kind() != SyntaxKind::Identifier && t.token != token.token
                    });
                    let first = it.next();
                    if first.as_ref().map_or(true, |f| f.token == token.token) {
                        return resolve_expression_scope(ctx).map(Into::into);
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
                        r.into()
                    })
                })?;
            }
            _ => (),
        }
    }
    None
}

fn with_insert_text(
    mut c: CompletionItem,
    ins_text: &str,
    client_caps: Option<&CompletionClientCapabilities>,
) -> CompletionItem {
    if client_caps
        .and_then(|caps| caps.completion_item.as_ref())
        .and_then(|caps| caps.snippet_support)
        .unwrap_or(false)
    {
        c.insert_text_format = Some(InsertTextFormat::SNIPPET);
        c.insert_text = Some(ins_text.to_string());
    }
    c
}

fn resolve_element_scope(
    element: syntax_nodes::Element,
    document_cache: &DocumentCache,
) -> Option<Vec<CompletionItem>> {
    let global_tr = document_cache.documents.global_type_registry.borrow();
    let tr = element
        .source_file()
        .and_then(|sf| document_cache.documents.get_document(sf.path()))
        .map(|doc| &doc.local_registry)
        .unwrap_or(&global_tr);
    let element_type = lookup_current_element_type((*element).clone(), tr).unwrap_or_default();
    Some(
        element_type
            .property_list()
            .into_iter()
            .map(|(k, t)| {
                let mut c = CompletionItem::new_simple(k, t.to_string());
                c.kind = Some(if matches!(t, Type::InferredCallback | Type::Callback { .. }) {
                    CompletionItemKind::METHOD
                } else {
                    CompletionItemKind::PROPERTY
                });
                c
            })
            .chain(element.PropertyDeclaration().map(|pr| {
                let mut c = CompletionItem::new_simple(
                    i_slint_compiler::parser::identifier_text(&pr.DeclaredIdentifier())
                        .unwrap_or_default(),
                    pr.Type().map(|t| t.text().into()).unwrap_or_else(|| "property".to_owned()),
                );
                c.kind = Some(CompletionItemKind::PROPERTY);
                c
            }))
            .chain(element.CallbackDeclaration().map(|cd| {
                let mut c = CompletionItem::new_simple(
                    i_slint_compiler::parser::identifier_text(&cd.DeclaredIdentifier())
                        .unwrap_or_default(),
                    "callback".into(),
                );
                c.kind = Some(CompletionItemKind::METHOD);
                c
            }))
            .chain(i_slint_compiler::typeregister::reserved_properties().filter_map(|(k, t)| {
                if matches!(t, Type::Function { .. }) {
                    return None;
                }
                let mut c = CompletionItem::new_simple(k.into(), t.to_string());
                c.kind = Some(if matches!(t, Type::InferredCallback | Type::Callback { .. }) {
                    CompletionItemKind::METHOD
                } else {
                    CompletionItemKind::PROPERTY
                });
                Some(c)
            }))
            .chain(tr.all_elements().into_iter().filter_map(|(k, t)| {
                match t {
                    ElementType::Component(c) if !c.is_global() => (),
                    ElementType::Builtin(b) if !b.is_internal && !b.is_global => (),
                    _ => return None,
                };
                let mut c = CompletionItem::new_simple(k, "element".into());
                c.kind = Some(CompletionItemKind::CLASS);
                Some(c)
            }))
            .collect(),
    )
}

fn resolve_expression_scope(lookup_context: &LookupCtx) -> Option<Vec<CompletionItem>> {
    let mut r = Vec::new();
    let global = i_slint_compiler::lookup::global_lookup();
    global.for_each_entry(lookup_context, &mut |str, expr| -> Option<()> {
        if str != "SlintInternal" {
            r.push(completion_item_from_expression(str, expr));
        }
        None
    });
    Some(r)
}

fn completion_item_from_expression(str: &str, lookup_result: LookupResult) -> CompletionItem {
    match lookup_result {
        LookupResult::Expression { expression, .. } => {
            let mut c = CompletionItem::new_simple(str.to_string(), expression.ty().to_string());
            c.kind = match expression {
                Expression::BoolLiteral(_) => Some(CompletionItemKind::CONSTANT),
                Expression::CallbackReference(_) => Some(CompletionItemKind::METHOD),
                Expression::PropertyReference(_) => Some(CompletionItemKind::PROPERTY),
                Expression::BuiltinFunctionReference(..) => Some(CompletionItemKind::FUNCTION),
                Expression::BuiltinMacroReference(..) => Some(CompletionItemKind::FUNCTION),
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
            let mut c = CompletionItem::new_simple(str.to_string(), e.name.clone());
            c.kind = Some(CompletionItemKind::ENUM);
            c
        }
        LookupResult::Namespace(_) => CompletionItem {
            label: str.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            ..CompletionItem::default()
        },
    }
}

fn resolve_type_scope(
    token: SyntaxToken,
    document_cache: &DocumentCache,
) -> Option<Vec<CompletionItem>> {
    let global_tr = document_cache.documents.global_type_registry.borrow();
    let tr = token
        .source_file()
        .and_then(|sf| document_cache.documents.get_document(sf.path()))
        .map(|doc| &doc.local_registry)
        .unwrap_or(&global_tr);
    Some(
        tr.all_types()
            .into_iter()
            .filter_map(|(k, t)| {
                t.is_property_type().then(|| {
                    let mut c = CompletionItem::new_simple(k, String::new());
                    c.kind = Some(CompletionItemKind::TYPE_PARAMETER);
                    c
                })
            })
            .collect(),
    )
}

fn complete_path_in_string(base: &Path, text: &str, offset: u32) -> Option<Vec<CompletionItem>> {
    if offset as usize > text.len() || offset == 0 {
        return None;
    }
    let mut text = text.strip_prefix('\"')?;
    text = &text[..(offset - 1) as usize];
    let path = if let Some(last_slash) = text.rfind('/') {
        base.parent()?.join(Path::new(&text[..last_slash]))
    } else {
        base.parent()?.to_owned()
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
