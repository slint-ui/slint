// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common::DocumentCache;
use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::{ElementType, EnumerationValue, Type};
use i_slint_compiler::lookup::{LookupObject, LookupResult};
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxToken};
use i_slint_compiler::pathutils::clean_path;
use smol_str::{SmolStr, ToSmolStr};
use std::path::Path;

pub enum TokenInfo {
    Type(Type),
    ElementType(ElementType),
    ElementRc(ElementRc),
    NamedReference(NamedReference),
    EnumerationValue(EnumerationValue),
    FileName(std::path::PathBuf),
    Image(std::path::PathBuf),
    LocalProperty(syntax_nodes::PropertyDeclaration),
    LocalCallback(syntax_nodes::CallbackDeclaration),
    /// This is like a NamedReference, but the element doesn't have an ElementRc because
    /// its enclosing component might not have been properly parsed
    IncompleteNamedReference(ElementType, SmolStr),
}

pub fn token_info(document_cache: &mut DocumentCache, token: SyntaxToken) -> Option<TokenInfo> {
    let mut node = token.parent();
    if node.kind() == SyntaxKind::AtImageUrl && token.kind() == SyntaxKind::StringLiteral {
        let path = i_slint_compiler::literals::unescape_string(token.text())?;
        let path = token.source_file.path().parent().map(|p| p.to_path_buf())?.join(path);

        return Some(TokenInfo::Image(clean_path(&path)));
    }

    loop {
        if let Some(n) = syntax_nodes::QualifiedName::new(node.clone()) {
            let parent = n.parent()?;
            return match parent.kind() {
                SyntaxKind::Type => {
                    let qual = i_slint_compiler::object_tree::QualifiedTypeName::from_node(n);
                    let doc = document_cache.get_document_for_source_file(&node.source_file)?;
                    Some(TokenInfo::Type(doc.local_registry.lookup_qualified(&qual.members)))
                }
                SyntaxKind::Element => {
                    let qual = i_slint_compiler::object_tree::QualifiedTypeName::from_node(n);
                    let doc = document_cache.get_document_for_source_file(&node.source_file)?;
                    Some(TokenInfo::ElementType(
                        doc.local_registry.lookup_element(&qual.to_string()).ok()?,
                    ))
                }
                SyntaxKind::Expression => {
                    if token.kind() != SyntaxKind::Identifier {
                        return None;
                    }
                    let lr = crate::util::with_lookup_ctx(document_cache, node, |ctx| {
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
                    match lr? {
                        LookupResult::Expression {
                            expression: Expression::ElementReference(e),
                            ..
                        } => Some(TokenInfo::ElementRc(e.upgrade()?)),
                        LookupResult::Expression {
                            expression:
                                Expression::CallbackReference(nr, _)
                                | Expression::PropertyReference(nr)
                                | Expression::FunctionReference(nr, _),
                            ..
                        } => Some(TokenInfo::NamedReference(nr)),
                        LookupResult::Expression {
                            expression: Expression::EnumerationValue(v),
                            ..
                        } => Some(TokenInfo::EnumerationValue(v)),
                        LookupResult::Enumeration(e) => Some(TokenInfo::Type(Type::Enumeration(e))),
                        _ => return None,
                    }
                }
                _ => None,
            };
        } else if let Some(n) = syntax_nodes::ImportIdentifier::new(node.clone()) {
            let doc = document_cache.get_document_for_source_file(&node.source_file)?;
            let imp_name = i_slint_compiler::typeloader::ImportedName::from_node(n);
            return Some(TokenInfo::ElementType(
                doc.local_registry.lookup_element(&imp_name.internal_name).ok()?,
            ));
        } else if let Some(n) = syntax_nodes::ExportSpecifier::new(node.clone()) {
            let doc = document_cache.get_document_for_source_file(&node.source_file)?;
            let (_, exp) = i_slint_compiler::object_tree::ExportedName::from_export_specifier(&n);
            return match doc.exports.find(exp.as_str())? {
                itertools::Either::Left(c) => {
                    Some(TokenInfo::ElementType(ElementType::Component(c)))
                }
                itertools::Either::Right(ty) => Some(TokenInfo::Type(ty)),
            };
        } else if matches!(node.kind(), SyntaxKind::ImportSpecifier | SyntaxKind::ExportModule) {
            let import_file = node
                .source_file
                .path()
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .join(node.child_text(SyntaxKind::StringLiteral)?.trim_matches('\"'));
            let import_file = clean_path(&import_file);
            return Some(TokenInfo::FileName(import_file));
        } else if syntax_nodes::BindingExpression::new(node.clone()).is_some() {
            // don't fallback to the Binding
            return None;
        } else if let Some(n) = syntax_nodes::Binding::new(node.clone()) {
            if token.kind() != SyntaxKind::Identifier {
                return None;
            }
            let prop_name = i_slint_compiler::parser::normalize_identifier(token.text());
            let element = syntax_nodes::Element::new(n.parent()?)?;
            if let Some(p) = element.PropertyDeclaration().find_map(|p| {
                (i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())? == prop_name)
                    .then_some(p)
            }) {
                return Some(TokenInfo::LocalProperty(p));
            }
            return find_property_declaration_in_base(document_cache, element, &prop_name);
        } else if let Some(n) = syntax_nodes::TwoWayBinding::new(node.clone()) {
            if token.kind() != SyntaxKind::Identifier {
                return None;
            }
            let prop_name = i_slint_compiler::parser::normalize_identifier(token.text());
            if prop_name != i_slint_compiler::parser::identifier_text(&n)? {
                return None;
            }
            let element = syntax_nodes::Element::new(n.parent()?)?;
            if let Some(p) = element.PropertyDeclaration().find_map(|p| {
                (i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())? == prop_name)
                    .then_some(p)
            }) {
                return Some(TokenInfo::LocalProperty(p));
            }
            return find_property_declaration_in_base(document_cache, element, &prop_name);
        } else if let Some(n) = syntax_nodes::CallbackConnection::new(node.clone()) {
            if token.kind() != SyntaxKind::Identifier {
                return None;
            }
            let prop_name = i_slint_compiler::parser::normalize_identifier(token.text());
            if prop_name != i_slint_compiler::parser::identifier_text(&n)? {
                return None;
            }
            let element = syntax_nodes::Element::new(n.parent()?)?;
            if let Some(p) = element.CallbackDeclaration().find_map(|p| {
                (i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())? == prop_name)
                    .then_some(p)
            }) {
                return Some(TokenInfo::LocalCallback(p));
            }
            return find_property_declaration_in_base(document_cache, element, &prop_name);
        } else if node.kind() == SyntaxKind::DeclaredIdentifier {
            let parent = node.parent()?;
            if parent.kind() == SyntaxKind::PropertyChangedCallback {
                if token.kind() != SyntaxKind::Identifier {
                    return None;
                }
                let prop_name = i_slint_compiler::parser::normalize_identifier(token.text());
                let element = syntax_nodes::Element::new(parent.parent()?)?;
                if let Some(p) = element.PropertyDeclaration().find_map(|p| {
                    (i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())?
                        == prop_name)
                        .then_some(p)
                }) {
                    return Some(TokenInfo::LocalProperty(p));
                }
                return find_property_declaration_in_base(document_cache, element, &prop_name);
            }
        }
        node = node.parent()?;
    }
}

/// Try to lookup the property `prop_name` in the base of the given Element
fn find_property_declaration_in_base(
    document_cache: &DocumentCache,
    element: syntax_nodes::Element,
    prop_name: &str,
) -> Option<TokenInfo> {
    let global_tr = document_cache.global_type_registry();
    let tr = element
        .source_file()
        .and_then(|sf| document_cache.get_document_for_source_file(sf))
        .map(|doc| &doc.local_registry)
        .unwrap_or(&global_tr);

    let element_type = crate::util::lookup_current_element_type((*element).clone(), tr)?;
    Some(TokenInfo::IncompleteNamedReference(element_type, prop_name.to_smolstr()))
}
