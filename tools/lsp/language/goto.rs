// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::DocumentCache;
use crate::util::{lookup_current_element_type, map_node_and_url, with_lookup_ctx};

use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::lookup::{LookupObject, LookupResult};
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken};
use i_slint_compiler::pathutils::clean_path;

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
                    let doc = document_cache.get_document_for_source_file(&node.source_file)?;
                    goto_type(&doc.local_registry.lookup_qualified(&qual.members))
                }
                SyntaxKind::Element => {
                    let qual = i_slint_compiler::object_tree::QualifiedTypeName::from_node(n);
                    let doc = document_cache.get_document_for_source_file(&node.source_file)?;
                    match doc.local_registry.lookup_element(&qual.to_string()) {
                        Ok(ElementType::Component(c)) => {
                            goto_node(&c.root_element.borrow().debug.first()?.node)
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
                        } => e.upgrade()?.borrow().debug.first()?.node.clone().into(),
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
            let doc = document_cache.get_document_for_source_file(&node.source_file)?;
            let imp_name = i_slint_compiler::typeloader::ImportedName::from_node(n);
            return match doc.local_registry.lookup_element(&imp_name.internal_name) {
                Ok(ElementType::Component(c)) => {
                    goto_node(&c.root_element.borrow().debug.first()?.node)
                }
                _ => None,
            };
        } else if let Some(n) = syntax_nodes::ExportSpecifier::new(node.clone()) {
            let doc = document_cache.get_document_for_source_file(&node.source_file)?;
            let (_, exp) = i_slint_compiler::object_tree::ExportedName::from_export_specifier(&n);
            return match doc.exports.find(exp.as_str())? {
                itertools::Either::Left(c) => {
                    goto_node(&c.root_element.borrow().debug.first()?.node)
                }
                itertools::Either::Right(ty) => goto_type(&ty),
            };
        } else if matches!(node.kind(), SyntaxKind::ImportSpecifier | SyntaxKind::ExportModule) {
            let import_file = node
                .source_file
                .path()
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .join(node.child_text(SyntaxKind::StringLiteral)?.trim_matches('\"'));
            let import_file = clean_path(&import_file);
            let doc = document_cache.get_document_by_path(&import_file)?;
            let doc_node = doc.node.clone()?;
            return goto_node(&doc_node);
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
                return goto_node(&p);
            }
            let n = find_property_declaration_in_base(document_cache, element, &prop_name)?;
            return goto_node(&n);
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
                return goto_node(&p);
            }
            let n = find_property_declaration_in_base(document_cache, element, &prop_name)?;
            return goto_node(&n);
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
                return goto_node(&p);
            }
            let n = find_property_declaration_in_base(document_cache, element, &prop_name)?;
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
    let global_tr = document_cache.global_type_registry();
    let tr = element
        .source_file()
        .and_then(|sf| document_cache.get_document_for_source_file(sf))
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

fn goto_type(ty: &Type) -> Option<GotoDefinitionResponse> {
    match ty {
        Type::Struct { node: Some(node), .. } => goto_node(node.parent().as_ref()?),
        Type::Enumeration(e) => goto_node(e.node.as_ref()?),
        _ => None,
    }
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

#[test]
fn test_goto_definition() {
    fn first_link(def: &GotoDefinitionResponse) -> &LocationLink {
        let GotoDefinitionResponse::Link(link) = def else { panic!("not a single link {def:?}") };
        link.first().unwrap()
    }

    let source = r#"
import { Button } from "std-widgets.slint";
component Abc {
    in property <string> hello;
}
export component Test {
    abc := Abc {
        hello: "foo";
    }
    btn := Button {
        text: abc.hello;
    }
    rec := Rectangle { }
}"#;

    let (mut dc, uri, _) = crate::language::test::loaded_document_cache(source.into());
    let doc = dc.get_document(&uri).unwrap().node.clone().unwrap();

    // Jump to the definition of Abc
    let offset = source.find("abc := Abc").unwrap() as u32;
    let token = crate::language::token_at_offset(&doc, offset + 8).unwrap();
    assert_eq!(token.text(), "Abc");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, uri);
    assert_eq!(link.target_range.start.line, 2);

    // Jump to the definition of abc
    let offset = source.find("text: abc.hello").unwrap() as u32;
    let token = crate::language::token_at_offset(&doc, offset + 7).unwrap();
    assert_eq!(token.text(), "abc");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, uri);
    assert_eq!(link.target_range.start.line, 6);

    // Jump to the definition of hello
    let offset = source.find("text: abc.hello").unwrap() as u32;
    let token = crate::language::token_at_offset(&doc, offset + 12).unwrap();
    assert_eq!(token.text(), "hello");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, uri);
    assert_eq!(link.target_range.start.line, 3);

    // Also jump to the definition of hello
    let offset = source.find("hello: \"foo\"").unwrap() as u32;
    let token = crate::language::token_at_offset(&doc, offset).unwrap();
    assert_eq!(token.text(), "hello");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, uri);
    assert_eq!(link.target_range.start.line, 3);

    // Rectangle is builtin and not accessible
    let offset = source.find("rec := ").unwrap() as u32;
    let token = crate::language::token_at_offset(&doc, offset + 8).unwrap();
    assert_eq!(token.text(), "Rectangle");
    assert!(goto_definition(&mut dc, token).is_none());

    // Button is builtin and not accessible
    let offset = source.find("btn := ").unwrap() as u32;
    let token = crate::language::token_at_offset(&doc, offset + 9).unwrap();
    assert_eq!(token.text(), "Button");
    assert!(goto_definition(&mut dc, token).is_none());
    let offset = source.find("text: abc.hello").unwrap() as u32;
    let token = crate::language::token_at_offset(&doc, offset).unwrap();
    assert_eq!(token.text(), "text");
    assert!(goto_definition(&mut dc, token).is_none());
}

#[test]
fn test_goto_definition_multi_files() {
    fn first_link(def: &GotoDefinitionResponse) -> &LocationLink {
        let GotoDefinitionResponse::Link(link) = def else { panic!("not a single link {def:?}") };
        link.first().unwrap()
    }

    let source1 = r#"
    export component Hello {
        in-out property <int> the_prop;
    }
    export struct AStruct {
        f: int
    }
    export component Another {
        callback xx;
    }
    "#;
    let (mut dc, url1, diags) = crate::language::test::loaded_document_cache(source1.into());
    for (u, ds) in diags {
        assert_eq!(ds, vec![], "errors in {u}");
    }
    let url2 = url1.join("../file2.slint").unwrap();
    let source2 = format!(
        r#"
        import {{ Hello }} from "{url1}";
        export {{ Another as A }} from "{url1}";
        export component Foo {{ h := Hello {{ the_prop: 42; }} }}
    "#,
        url1 = url1.to_file_path().unwrap().display()
    );
    let diags = spin_on::spin_on(crate::language::reload_document_impl(
        None,
        source2.clone(),
        url2.clone(),
        Some(43),
        &mut dc,
    ));
    for (u, ds) in diags {
        assert_eq!(ds, vec![], "errors in {u}");
    }
    let doc2 = dc.get_document(&url2).unwrap().node.clone().unwrap();

    let offset = source2.find("h := Hello").unwrap() as u32;
    let token = crate::language::token_at_offset(&doc2, offset + 8).unwrap();
    assert_eq!(token.text(), "Hello");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 1);

    let offset = source2.find("the_prop: 42").unwrap() as u32;
    let token = crate::language::token_at_offset(&doc2, offset).unwrap();
    assert_eq!(token.text(), "the_prop");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 2);

    let offset = source2.find("Hello } from ").unwrap() as u32;
    // check the string literal
    let token = crate::language::token_at_offset(&doc2, offset + 20).unwrap();
    assert_eq!(token.kind(), SyntaxKind::StringLiteral);
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 0);
    // check the identifier
    let token = crate::language::token_at_offset(&doc2, offset).unwrap();
    assert_eq!(token.text(), "Hello");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 1);

    let offset = source2.find("Another as A } from ").unwrap() as u32;
    // check the string literal
    let token = crate::language::token_at_offset(&doc2, offset + 25).unwrap();
    assert_eq!(token.kind(), SyntaxKind::StringLiteral);
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 0);
    // check the identifier
    let token = crate::language::token_at_offset(&doc2, offset).unwrap();
    assert_eq!(token.text(), "Another");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 7);
}
