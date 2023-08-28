// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use super::DocumentCache;
use crate::util::{lookup_current_element_type, map_node_and_url, with_lookup_ctx};

use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::lookup::{LookupObject, LookupResult};
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken};

use lsp_types::{GotoDefinitionResponse, LocationLink, Range};
use std::path::Path;

pub fn goto_definition(
    document_cache: &mut DocumentCache,
    token: SyntaxToken,
) -> Option<GotoDefinitionResponse> {
    let mut node = token.parent();
    loop {
        if let Some(n) = syntax_nodes::QualifiedName::new(node.clone()) {
            let parent = n.parent()?;
            return match parent.kind() {
                SyntaxKind::Type => {
                    let qual = i_slint_compiler::object_tree::QualifiedTypeName::from_node(n);
                    let doc = document_cache.documents.get_document(node.source_file.path())?;
                    match doc.local_registry.lookup_qualified(&qual.members) {
                        Type::Struct { node: Some(node), .. } => goto_node(node.parent().as_ref()?),
                        Type::Enumeration(e) => goto_node(e.node.as_ref()?),
                        _ => None,
                    }
                }
                SyntaxKind::Element => {
                    let qual = i_slint_compiler::object_tree::QualifiedTypeName::from_node(n);
                    let doc = document_cache.documents.get_document(node.source_file.path())?;
                    match doc.local_registry.lookup_element(&qual.to_string()) {
                        Ok(ElementType::Component(c)) => {
                            goto_node(c.root_element.borrow().node.as_ref()?)
                        }
                        _ => None,
                    }
                }
                SyntaxKind::Expression => {
                    if token.kind() != SyntaxKind::Identifier {
                        return None;
                    }
                    let lr = with_lookup_ctx(document_cache, node, |ctx| {
                        let mut it = n
                            .children_with_tokens()
                            .filter_map(|t| t.into_token())
                            .filter(|t| t.kind() == SyntaxKind::Identifier);
                        let mut cur_tok = it.next()?;
                        let first_str =
                            i_slint_compiler::parser::normalize_identifier(cur_tok.text());
                        let global = i_slint_compiler::lookup::global_lookup();
                        let mut expr_it = global.lookup(ctx, &first_str)?;
                        while cur_tok.token != token.token {
                            cur_tok = it.next()?;
                            let str =
                                i_slint_compiler::parser::normalize_identifier(cur_tok.text());
                            expr_it = expr_it.lookup(ctx, &str)?;
                        }
                        Some(expr_it)
                    })?;
                    let gn = match lr? {
                        LookupResult::Expression {
                            expression: Expression::ElementReference(e),
                            ..
                        } => e.upgrade()?.borrow().node.clone()?.into(),
                        LookupResult::Expression {
                            expression:
                                Expression::CallbackReference(nr, _)
                                | Expression::PropertyReference(nr)
                                | Expression::FunctionReference(nr, _),
                            ..
                        } => {
                            let mut el = nr.element();
                            loop {
                                if let Some(x) = el.borrow().property_declarations.get(nr.name()) {
                                    break x.node.clone()?;
                                }
                                let base = el.borrow().base_type.clone();
                                if let ElementType::Component(c) = base {
                                    el = c.root_element.clone();
                                } else {
                                    return None;
                                }
                            }
                        }
                        LookupResult::Expression {
                            expression: Expression::EnumerationValue(v),
                            ..
                        } => {
                            // FIXME: this goes to the enum definition instead of the value definition.
                            v.enumeration.node.clone()?.into()
                        }
                        LookupResult::Enumeration(e) => e.node.clone()?.into(),
                        _ => return None,
                    };
                    goto_node(&gn)
                }
                _ => None,
            };
        } else if let Some(n) = syntax_nodes::ImportIdentifier::new(node.clone()) {
            let doc = document_cache.documents.get_document(node.source_file.path())?;
            let imp_name = i_slint_compiler::typeloader::ImportedName::from_node(n);
            return match doc.local_registry.lookup_element(&imp_name.internal_name) {
                Ok(ElementType::Component(c)) => goto_node(c.root_element.borrow().node.as_ref()?),
                _ => None,
            };
        } else if let Some(n) = syntax_nodes::ImportSpecifier::new(node.clone()) {
            let import_file = node
                .source_file
                .path()
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .join(n.child_text(SyntaxKind::StringLiteral)?.trim_matches('\"'));
            let import_file = dunce::canonicalize(&import_file).unwrap_or(import_file);
            let doc = document_cache.documents.get_document(&import_file)?;
            let doc_node = doc.node.clone()?;
            return goto_node(&doc_node);
        } else if syntax_nodes::BindingExpression::new(node.clone()).is_some() {
            // don't fallback to the Binding
            return None;
        } else if let Some(n) = syntax_nodes::Binding::new(node.clone()) {
            if token.kind() != SyntaxKind::Identifier {
                return None;
            }
            let prop_name = token.text();
            let element = syntax_nodes::Element::new(n.parent()?)?;
            if let Some(p) = element.PropertyDeclaration().find_map(|p| {
                (i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())? == prop_name)
                    .then_some(p)
            }) {
                return goto_node(&p);
            }
            let n = find_property_declaration_in_base(document_cache, element, prop_name)?;
            return goto_node(&n);
        } else if let Some(n) = syntax_nodes::TwoWayBinding::new(node.clone()) {
            if token.kind() != SyntaxKind::Identifier {
                return None;
            }
            let prop_name = token.text();
            if prop_name != n.child_text(SyntaxKind::Identifier)? {
                return None;
            }
            let element = syntax_nodes::Element::new(n.parent()?)?;
            if let Some(p) = element.PropertyDeclaration().find_map(|p| {
                (i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())? == prop_name)
                    .then_some(p)
            }) {
                return goto_node(&p);
            }
            let n = find_property_declaration_in_base(document_cache, element, prop_name)?;
            return goto_node(&n);
        } else if let Some(n) = syntax_nodes::CallbackConnection::new(node.clone()) {
            if token.kind() != SyntaxKind::Identifier {
                return None;
            }
            let prop_name = token.text();
            if prop_name != n.child_text(SyntaxKind::Identifier)? {
                return None;
            }
            let element = syntax_nodes::Element::new(n.parent()?)?;
            if let Some(p) = element.CallbackDeclaration().find_map(|p| {
                (i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())? == prop_name)
                    .then_some(p)
            }) {
                return goto_node(&p);
            }
            let n = find_property_declaration_in_base(document_cache, element, prop_name)?;
            return goto_node(&n);
        }
        node = node.parent()?;
    }
}

/// Try to lookup the property `prop_name` in the base of the given Element
fn find_property_declaration_in_base(
    document_cache: &DocumentCache,
    element: syntax_nodes::Element,
    prop_name: &str,
) -> Option<SyntaxNode> {
    let global_tr = document_cache.documents.global_type_registry.borrow();
    let tr = element
        .source_file()
        .and_then(|sf| document_cache.documents.get_document(sf.path()))
        .map(|doc| &doc.local_registry)
        .unwrap_or(&global_tr);

    let mut element_type = lookup_current_element_type((*element).clone(), tr)?;
    while let ElementType::Component(com) = element_type {
        if let Some(p) = com.root_element.borrow().property_declarations.get(prop_name) {
            return p.node.clone();
        }
        element_type = com.root_element.borrow().base_type.clone();
    }
    None
}

fn goto_node(node: &SyntaxNode) -> Option<GotoDefinitionResponse> {
    let (target_uri, range) = map_node_and_url(node)?;
    let range = Range::new(range.start, range.start); // Shrink range to a position:-)
    Some(GotoDefinitionResponse::Link(vec![LocationLink {
        origin_selection_range: None,
        target_uri,
        target_range: range,
        target_selection_range: range,
    }]))
}
