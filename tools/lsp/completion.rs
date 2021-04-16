/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use super::{lookup_current_element_type, DocumentCache};
use lsp_types::{CompletionItem, CompletionItemKind};
use sixtyfps_compilerlib::diagnostics::Spanned;
use sixtyfps_compilerlib::langtype::Type;
use sixtyfps_compilerlib::parser::{syntax_nodes, SyntaxKind, SyntaxNode};

pub(crate) fn completion_at(
    document_cache: &DocumentCache,
    token: SyntaxNode,
) -> Option<Vec<CompletionItem>> {
    match token.kind() {
        SyntaxKind::Element => {
            let element = syntax_nodes::Element::from(token.clone());
            let global_tr = document_cache.documents.global_type_registry.borrow();
            let tr = token
                .source_file()
                .and_then(|sf| document_cache.documents.get_document(sf.path()))
                .map(|doc| &doc.local_registry)
                .unwrap_or(&global_tr);
            let element_type = lookup_current_element_type(token, tr).unwrap_or_default();
            return Some(
                element_type
                    .property_list()
                    .into_iter()
                    .map(|(k, t)| {
                        let mut c = CompletionItem::new_simple(k, t.to_string());
                        c.kind = Some(if matches!(t, Type::Callback { .. }) {
                            CompletionItemKind::Method
                        } else {
                            CompletionItemKind::Property
                        });
                        c
                    })
                    .chain(element.PropertyDeclaration().map(|pr| {
                        let mut c = CompletionItem::new_simple(
                            sixtyfps_compilerlib::parser::identifier_text(&pr.DeclaredIdentifier())
                                .unwrap_or_default(),
                            pr.Type().text().into(),
                        );
                        c.kind = Some(CompletionItemKind::Property);
                        c
                    }))
                    .chain(element.CallbackDeclaration().map(|cd| {
                        let mut c = CompletionItem::new_simple(
                            sixtyfps_compilerlib::parser::identifier_text(&cd.DeclaredIdentifier())
                                .unwrap_or_default(),
                            "callback".into(),
                        );
                        c.kind = Some(CompletionItemKind::Method);
                        c
                    }))
                    .chain(tr.all_types().into_iter().filter_map(|(k, t)| {
                        if !matches!(t, Type::Component(_) | Type::Builtin(_)) {
                            return None;
                        } else {
                            let mut c = CompletionItem::new_simple(k, "element".into());
                            c.kind = Some(CompletionItemKind::Class);
                            Some(c)
                        }
                    }))
                    .collect(),
            );
        }
        _ => return None,
    }
}
