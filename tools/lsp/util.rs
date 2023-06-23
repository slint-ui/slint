// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use crate::DocumentCache;

use i_slint_compiler::diagnostics::{DiagnosticLevel, SourceFile, Spanned};
use i_slint_compiler::langtype::ElementType;
use i_slint_compiler::lookup::LookupCtx;
use i_slint_compiler::object_tree;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken};
use i_slint_compiler::parser::{TextRange, TextSize};
use i_slint_compiler::typeregister::TypeRegister;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::UrlWasm;

pub fn map_node_and_url(node: &SyntaxNode) -> Option<(lsp_types::Url, lsp_types::Range)> {
    let range = node.text_range();
    node.source_file().map(|sf| {
        (
            lsp_types::Url::from_file_path(sf.path()).unwrap_or_else(|_| invalid_url()),
            map_range(sf, range),
        )
    })
}

pub fn map_node(node: &SyntaxNode) -> Option<lsp_types::Range> {
    let range = node.text_range();
    node.source_file().map(|sf| map_range(sf, range))
}

pub fn map_token(token: &SyntaxToken) -> Option<lsp_types::Range> {
    let range = token.text_range();
    token.parent().source_file().map(|sf| map_range(sf, range))
}

pub fn map_position(sf: &SourceFile, pos: TextSize) -> lsp_types::Position {
    let (line, column) = sf.line_column(pos.into());
    lsp_types::Position::new((line as u32).saturating_sub(1), (column as u32).saturating_sub(1))
}

pub fn map_range(sf: &SourceFile, range: TextRange) -> lsp_types::Range {
    lsp_types::Range::new(map_position(sf, range.start()), map_position(sf, range.end()))
}

pub fn invalid_url() -> lsp_types::Url {
    lsp_types::Url::parse("invalid:///").unwrap()
}

#[test]
fn test_invalid_url() {
    assert_eq!(invalid_url().scheme(), "invalid");
}

/// Given a node within an element, return the Type for the Element under that node.
/// (If node is an element, return the Type for that element, otherwise the type of the element under it)
/// Will return `Foo` in the following example where `|` is the cursor.
///
/// ```text
/// Hello := A {
///   B {
///      Foo {
///        |
///      }
///   }
/// }
/// ```
pub fn lookup_current_element_type(mut node: SyntaxNode, tr: &TypeRegister) -> Option<ElementType> {
    while node.kind() != SyntaxKind::Element {
        if let Some(parent) = node.parent() {
            node = parent
        } else {
            return None;
        }
    }

    let parent = node.parent()?;
    if parent.kind() == SyntaxKind::Component
        && parent.child_text(SyntaxKind::Identifier).map_or(false, |x| x == "global")
    {
        return Some(ElementType::Global);
    }
    let parent = lookup_current_element_type(parent, tr).unwrap_or_default();
    let qualname = object_tree::QualifiedTypeName::from_node(
        syntax_nodes::Element::from(node).QualifiedName()?,
    );
    parent.lookup_type_for_child_element(&qualname.to_string(), tr).ok()
}

/// Run the function with the LookupCtx associated with the token
pub fn with_lookup_ctx<R>(
    document_cache: &DocumentCache,
    node: SyntaxNode,
    f: impl FnOnce(&mut LookupCtx) -> R,
) -> Option<R> {
    let (element, prop_name) = lookup_expression_context(node)?;
    with_property_lookup_ctx::<R>(document_cache, &element, &prop_name, f)
}

/// Run the function with the LookupCtx associated with the token
pub fn with_property_lookup_ctx<R>(
    document_cache: &DocumentCache,
    element: &syntax_nodes::Element,
    prop_name: &str,
    f: impl FnOnce(&mut LookupCtx) -> R,
) -> Option<R> {
    let global_tr = document_cache.documents.global_type_registry.borrow();
    let tr = element
        .source_file()
        .and_then(|sf| document_cache.documents.get_document(sf.path()))
        .map(|doc| &doc.local_registry)
        .unwrap_or(&global_tr);

    let component = {
        let mut n = element.parent()?;
        loop {
            if let Some(component) = syntax_nodes::Component::new(n.clone()) {
                break component;
            }
            n = n.parent()?;
        }
    };

    let mut scope = Vec::new();
    let component = i_slint_compiler::parser::identifier_text(&component.DeclaredIdentifier())
        .and_then(|component_name| tr.lookup_element(&component_name).ok())?;
    if let ElementType::Component(c) = component {
        let mut it = c.root_element.clone();
        let offset = element.text_range().start();
        loop {
            scope.push(it.clone());
            if let Some(c) = it.clone().borrow().children.iter().find(|c| {
                c.borrow().node.as_ref().map_or(false, |n| n.text_range().contains(offset))
            }) {
                it = c.clone();
            } else {
                break;
            }
        }
    };

    let ty = element
        .PropertyDeclaration()
        .find_map(|p| {
            (i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())? == prop_name)
                .then_some(p)
        })
        .and_then(|p| p.Type())
        .map(|n| object_tree::type_from_node(n, &mut Default::default(), tr))
        .or_else(|| scope.last().map(|e| e.borrow().lookup_property(prop_name).property_type));

    let mut build_diagnostics = Default::default();
    let mut lookup_context = LookupCtx::empty_context(tr, &mut build_diagnostics);
    lookup_context.property_name = Some(prop_name);
    lookup_context.property_type = ty.unwrap_or_default();
    lookup_context.component_scope = &scope;

    if let Some(cb) = element
        .CallbackConnection()
        .find(|p| i_slint_compiler::parser::identifier_text(p).map_or(false, |x| x == prop_name))
    {
        lookup_context.arguments = cb
            .DeclaredIdentifier()
            .flat_map(|a| i_slint_compiler::parser::identifier_text(&a))
            .collect();
    } else if let Some(f) = element.Function().find(|p| {
        i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())
            .map_or(false, |x| x == prop_name)
    }) {
        lookup_context.arguments = f
            .ArgumentDeclaration()
            .flat_map(|a| i_slint_compiler::parser::identifier_text(&a.DeclaredIdentifier()))
            .collect();
    }
    Some(f(&mut lookup_context))
}

/// Return the element and property name in which we are
fn lookup_expression_context(mut n: SyntaxNode) -> Option<(syntax_nodes::Element, String)> {
    let (element, prop_name) = loop {
        if let Some(decl) = syntax_nodes::PropertyDeclaration::new(n.clone()) {
            let prop_name = i_slint_compiler::parser::identifier_text(&decl.DeclaredIdentifier())?;
            let element = syntax_nodes::Element::new(n.parent()?)?;
            break (element, prop_name);
        }
        match n.kind() {
            SyntaxKind::Binding | SyntaxKind::TwoWayBinding | SyntaxKind::CallbackConnection => {
                let prop_name = i_slint_compiler::parser::identifier_text(&n)?;
                let element = syntax_nodes::Element::new(n.parent()?)?;
                break (element, prop_name);
            }
            SyntaxKind::Function => {
                let prop_name = i_slint_compiler::parser::identifier_text(
                    &n.child_node(SyntaxKind::DeclaredIdentifier)?,
                )?;
                let element = syntax_nodes::Element::new(n.parent()?)?;
                break (element, prop_name);
            }
            SyntaxKind::ConditionalElement | SyntaxKind::RepeatedElement => {
                let element = syntax_nodes::Element::new(n.parent()?)?;
                break (element, "$model".to_string());
            }
            SyntaxKind::Element => {
                // oops: missed it
                let element = syntax_nodes::Element::new(n)?;
                break (element, String::new());
            }
            _ => n = n.parent()?,
        }
    };
    Some((element, prop_name))
}

pub fn to_lsp_diag(d: &i_slint_compiler::diagnostics::Diagnostic) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic::new(
        to_range(d.line_column()),
        Some(to_lsp_diag_level(d.level())),
        None,
        None,
        d.message().to_owned(),
        None,
        None,
    )
}

fn to_range(span: (usize, usize)) -> lsp_types::Range {
    let pos = lsp_types::Position::new(
        (span.0 as u32).saturating_sub(1),
        (span.1 as u32).saturating_sub(1),
    );
    lsp_types::Range::new(pos, pos)
}

fn to_lsp_diag_level(level: DiagnosticLevel) -> lsp_types::DiagnosticSeverity {
    match level {
        DiagnosticLevel::Error => lsp_types::DiagnosticSeverity::ERROR,
        DiagnosticLevel::Warning => lsp_types::DiagnosticSeverity::WARNING,
        _ => lsp_types::DiagnosticSeverity::INFORMATION,
    }
}
