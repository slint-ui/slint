// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_compiler::diagnostics::{DiagnosticLevel, SourceFile, Spanned};
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::lookup::LookupCtx;
use i_slint_compiler::object_tree;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken};
use i_slint_compiler::parser::{TextRange, TextSize};
use i_slint_compiler::typeloader::TypeLoader;
use i_slint_compiler::typeregister::TypeRegister;

use crate::common;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::UrlWasm;

pub fn map_node_and_url(node: &SyntaxNode) -> Option<(lsp_types::Url, lsp_types::Range)> {
    let sf = node.source_file()?;
    let range = node.text_range();
    Some((lsp_types::Url::from_file_path(sf.path()).ok()?, map_range(sf, range)))
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

// Find the last token that is not a Whitespace in a `SyntaxNode`. May return
// `None` if the node contains no tokens or they are all Whitespace.
pub fn last_non_ws_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    let mut last_non_ws = None;
    let mut token = node.first_token();
    while let Some(t) = token {
        if t.text_range().end() > node.text_range().end() {
            break;
        }

        if t.kind() != SyntaxKind::Whitespace {
            last_non_ws = Some(t.clone());
        }
        token = t.next_token();
    }
    last_non_ws
}

// Find the indentation of the element node itself as well as the indentation of properties inside the
// element. Returns the element indent followed by the block indent
pub fn find_element_indent(element: &common::ElementRcNode) -> Option<String> {
    let mut token = element.with_element_node(|node| node.first_token()?.prev_token());
    while let Some(t) = token {
        if t.kind() == SyntaxKind::Whitespace && t.text().contains('\n') {
            return t.text().split('\n').last().map(|s| s.to_owned());
        }
        token = t.prev_token();
    }
    None
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

#[derive(Debug)]
pub struct ExpressionContextInfo {
    element: syntax_nodes::Element,
    property_name: String,
    is_animate: bool,
}

impl ExpressionContextInfo {
    pub fn new(element: syntax_nodes::Element, property_name: String, is_animate: bool) -> Self {
        ExpressionContextInfo { element, property_name, is_animate }
    }
}

/// Run the function with the LookupCtx associated with the token
pub fn with_lookup_ctx<R>(
    type_loader: &TypeLoader,
    node: SyntaxNode,
    f: impl FnOnce(&mut LookupCtx) -> R,
) -> Option<R> {
    let expr_context_info = lookup_expression_context(node)?;
    with_property_lookup_ctx::<R>(type_loader, &expr_context_info, f)
}

/// Run the function with the LookupCtx associated with the token
pub fn with_property_lookup_ctx<R>(
    type_loader: &TypeLoader,
    expr_context_info: &ExpressionContextInfo,
    f: impl FnOnce(&mut LookupCtx) -> R,
) -> Option<R> {
    let (element, prop_name, is_animate) = (
        &expr_context_info.element,
        expr_context_info.property_name.as_str(),
        expr_context_info.is_animate,
    );
    let global_tr = type_loader.global_type_registry.borrow();
    let tr = element
        .source_file()
        .and_then(|sf| type_loader.get_document(sf.path()))
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
                c.borrow().debug.first().map_or(false, |n| n.0.text_range().contains(offset))
            }) {
                it = c.clone();
            } else {
                break;
            }
        }
    };

    let mut ty = element
        .PropertyDeclaration()
        .find_map(|p| {
            (i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())? == prop_name)
                .then_some(p)
        })
        .and_then(|p| p.Type())
        .map(|n| object_tree::type_from_node(n, &mut Default::default(), tr))
        .or_else(|| scope.last().map(|e| e.borrow().lookup_property(prop_name).property_type));

    // try to match properties from `PropertyAnimation`
    if is_animate {
        ty = global_tr
            .property_animation_type_for_property(Type::Float32)
            .property_list()
            .iter()
            .find_map(|(p, t)| if p.as_str() == prop_name { Some(t.clone()) } else { None })
    }

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
fn lookup_expression_context(mut n: SyntaxNode) -> Option<ExpressionContextInfo> {
    let (element, prop_name, is_animate) = loop {
        if let Some(decl) = syntax_nodes::PropertyDeclaration::new(n.clone()) {
            let prop_name = i_slint_compiler::parser::identifier_text(&decl.DeclaredIdentifier())?;
            let element = syntax_nodes::Element::new(n.parent()?)?;
            break (element, prop_name, false);
        }
        match n.kind() {
            SyntaxKind::Binding | SyntaxKind::TwoWayBinding | SyntaxKind::CallbackConnection => {
                let mut parent = n.parent()?;
                if parent.kind() == SyntaxKind::PropertyAnimation {
                    let prop_name = i_slint_compiler::parser::identifier_text(&n)?;
                    let element = syntax_nodes::Element::new(parent.parent()?)?;
                    break (element, prop_name, true);
                } else {
                    let prop_name =
                        i_slint_compiler::parser::identifier_text(&n).unwrap_or_default();
                    loop {
                        if let Some(element) = syntax_nodes::Element::new(parent.clone()) {
                            return Some(ExpressionContextInfo::new(element, prop_name, false));
                        }
                        parent = parent.parent()?;
                    }
                }
            }
            SyntaxKind::Function => {
                let prop_name = i_slint_compiler::parser::identifier_text(
                    &n.child_node(SyntaxKind::DeclaredIdentifier)?,
                )?;
                let element = syntax_nodes::Element::new(n.parent()?)?;
                break (element, prop_name, false);
            }
            SyntaxKind::ConditionalElement | SyntaxKind::RepeatedElement => {
                let element = syntax_nodes::Element::new(n.parent()?)?;
                break (element, "$model".to_string(), false);
            }
            SyntaxKind::Element => {
                // oops: missed it
                let element = syntax_nodes::Element::new(n)?;
                break (element, String::new(), false);
            }
            _ => n = n.parent()?,
        }
    };
    Some(ExpressionContextInfo::new(element, prop_name, is_animate))
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::language;
    use crate::language::test::loaded_document_cache;

    #[test]
    fn test_find_element_indent() {
        let (dc, url, _) = loaded_document_cache(
            r#"component MainWindow inherits Window {
    VerticalBox {
        label := Text { text: "text"; }
    }
}"#
            .to_string(),
        );

        let window =
            language::element_at_position(&dc.documents, &url, &lsp_types::Position::new(0, 30));
        assert_eq!(find_element_indent(&window.unwrap()), None);

        let vbox =
            language::element_at_position(&dc.documents, &url, &lsp_types::Position::new(1, 4));
        assert_eq!(find_element_indent(&vbox.unwrap()), Some("    ".to_string()));

        let label =
            language::element_at_position(&dc.documents, &url, &lsp_types::Position::new(2, 17));
        assert_eq!(find_element_indent(&label.unwrap()), Some("        ".to_string()));
    }
}
