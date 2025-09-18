// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::diagnostics::{DiagnosticLevel, SourceFile, Spanned};
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::lookup::LookupCtx;
use i_slint_compiler::object_tree::{self, type_from_node};
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken};
use i_slint_compiler::parser::{TextRange, TextSize};
use i_slint_compiler::typeregister::TypeRegister;
use smol_str::SmolStr;

use crate::common;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::UrlWasm;

/// Get the `TextRange` of a `node`, excluding any trailing whitespace tokens.
pub fn node_range_without_trailing_ws(node: &SyntaxNode) -> TextRange {
    let range = node.text_range();
    // shorten range to not include trailing WS:
    TextRange::new(
        range.start(),
        last_non_ws_token(node).map(|t| t.text_range().end()).unwrap_or(range.end()),
    )
}

/// Map a `node` to its `Url` and a `Range` of characters covered by the `node`
///
/// This will exclude trailing whitespaces.
pub fn node_to_url_and_lsp_range(node: &SyntaxNode) -> Option<(lsp_types::Url, lsp_types::Range)> {
    let path = node.source_file.path();
    Some((lsp_types::Url::from_file_path(path).ok()?, node_to_lsp_range(node)))
}

/// Map a `node` to the `Range` of characters covered by the `node`
pub fn node_to_lsp_range(node: &SyntaxNode) -> lsp_types::Range {
    let range = node.text_range();
    text_range_to_lsp_range(&node.source_file, range)
}

/// Map a `token` to the `Range` of characters covered by the `token`
pub fn token_to_lsp_range(token: &SyntaxToken) -> lsp_types::Range {
    let range = token.text_range();
    text_range_to_lsp_range(&token.parent().source_file, range)
}

/// Convert a `TextSize` to a `Position` for use in the LSP
pub fn text_size_to_lsp_position(sf: &SourceFile, pos: TextSize) -> lsp_types::Position {
    let (line, column) = sf.line_column(pos.into());
    lsp_types::Position::new((line as u32).saturating_sub(1), (column as u32).saturating_sub(1))
}

/// Convert a `TextRange` to a `Range` for use in the LSP
pub fn text_range_to_lsp_range(sf: &SourceFile, range: TextRange) -> lsp_types::Range {
    lsp_types::Range::new(
        text_size_to_lsp_position(sf, range.start()),
        text_size_to_lsp_position(sf, range.end()),
    )
}

/// Convert a `Position` from the LSP into a `TextSize`
pub fn lsp_position_to_text_size(sf: &SourceFile, position: lsp_types::Position) -> TextSize {
    (sf.offset(
        usize::try_from(position.line).unwrap() + 1,
        usize::try_from(position.character).unwrap() + 1,
    ) as u32)
        .into()
}

/// Convert a `Range` from the LSP into a `TextRange`
pub fn lsp_range_to_text_range(sf: &SourceFile, range: lsp_types::Range) -> TextRange {
    TextRange::new(
        lsp_position_to_text_size(sf, range.start),
        lsp_position_to_text_size(sf, range.end),
    )
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

        if t.kind() != SyntaxKind::Whitespace && t.kind() != SyntaxKind::Eof {
            last_non_ws = Some(t.clone());
        }
        token = t.next_token();
    }
    last_non_ws
}

// Find the indentation of the element node itself as well as the indentation of properties inside the
// element. Returns the element indent.
pub fn find_element_indent(element: &common::ElementRcNode) -> Option<String> {
    let mut token = element.with_element_node(|node| node.first_token()?.prev_token());
    while let Some(t) = token {
        if t.kind() == SyntaxKind::Whitespace && t.text().contains('\n') {
            return t.text().split('\n').next_back().map(|s| s.to_owned());
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
        && parent.child_text(SyntaxKind::Identifier).is_some_and(|x| x == "global")
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
    property_name: SmolStr,
    is_animate: bool,
}

impl ExpressionContextInfo {
    pub fn new(element: syntax_nodes::Element, property_name: SmolStr, is_animate: bool) -> Self {
        ExpressionContextInfo { element, property_name, is_animate }
    }
}

/// Run the function with the LookupCtx associated with the token
pub fn with_lookup_ctx<R>(
    document_cache: &common::DocumentCache,
    node: SyntaxNode,
    to_offset: Option<TextSize>,
    f: impl FnOnce(&mut LookupCtx) -> R,
) -> Option<R> {
    let expr_context_info = lookup_expression_context(node)?;
    with_property_lookup_ctx::<R>(document_cache, &expr_context_info, to_offset, f)
}

/// Run the function with the LookupCtx associated with the token
pub fn with_property_lookup_ctx<R>(
    document_cache: &common::DocumentCache,
    expr_context_info: &ExpressionContextInfo,
    to_offset: Option<TextSize>,
    f: impl FnOnce(&mut LookupCtx) -> R,
) -> Option<R> {
    let (element, prop_name, is_animate) = (
        &expr_context_info.element,
        expr_context_info.property_name.as_str(),
        expr_context_info.is_animate,
    );
    let global_tr = document_cache.global_type_registry();
    let tr = element
        .source_file()
        .and_then(|sf| document_cache.get_document_for_source_file(sf))
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
                c.borrow().debug.first().is_some_and(|n| n.node.text_range().contains(offset))
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
    lookup_context.current_token = Some((**element).clone().into());

    if let Some(cb) = element
        .CallbackConnection()
        .find(|p| i_slint_compiler::parser::identifier_text(p).is_some_and(|x| x == prop_name))
    {
        lookup_context.arguments = cb
            .DeclaredIdentifier()
            .flat_map(|a| i_slint_compiler::parser::identifier_text(&a))
            .collect();
        if let Some(block) = cb.CodeBlock() {
            add_codeblock_local_variables(&block, to_offset, &mut lookup_context);
        }
    } else if let Some(f) = element.Function().find(|p| {
        i_slint_compiler::parser::identifier_text(&p.DeclaredIdentifier())
            .is_some_and(|x| x == prop_name)
    }) {
        lookup_context.arguments = f
            .ArgumentDeclaration()
            .flat_map(|a| i_slint_compiler::parser::identifier_text(&a.DeclaredIdentifier()))
            .collect();

        add_codeblock_local_variables(&f.CodeBlock(), to_offset, &mut lookup_context);
    } else if let Some(cb) = element
        .PropertyChangedCallback()
        .find(|p| i_slint_compiler::parser::identifier_text(p).is_some_and(|x| x == prop_name))
    {
        if let Some(block) = cb.CodeBlock() {
            add_codeblock_local_variables(&block, to_offset, &mut lookup_context);
        }
    } else if let Some(b) = element
        .Binding()
        .find(|p| i_slint_compiler::parser::identifier_text(p).is_some_and(|x| x == prop_name))
    {
        if let Some(cb) = b.BindingExpression().CodeBlock() {
            add_codeblock_local_variables(&cb, to_offset, &mut lookup_context);
        }
    }

    Some(f(&mut lookup_context))
}

// recursively add local variables from a code block to the context
fn add_codeblock_local_variables(
    code_block: &syntax_nodes::CodeBlock,
    to_offset: Option<TextSize>,
    ctx: &mut LookupCtx,
) {
    if let Some(offset) = to_offset {
        if !code_block.text_range().contains(offset) {
            return; // out of scope
        }
    }

    let locals = code_block
        .LetStatement()
        .take_while(|e| to_offset.is_none_or(|offset| e.text_range().start() < offset))
        .map(|e| {
            let value = Expression::from_expression_node(e.Expression(), ctx);
            let ty = e
                .Type()
                .map(|ty| type_from_node(ty, ctx.diag, ctx.type_register))
                .unwrap_or_else(|| value.ty());
            (
                i_slint_compiler::parser::identifier_text(&e.DeclaredIdentifier())
                    .unwrap_or_default(),
                ty,
            )
        })
        .collect();

    ctx.local_variables.push(locals);

    code_block.Expression().for_each(|e| {
        if let Some(cb) = e.CodeBlock() {
            add_codeblock_local_variables(&cb, to_offset, ctx);
        }
    })
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
            SyntaxKind::Binding
            | SyntaxKind::TwoWayBinding
            | SyntaxKind::CallbackConnection
            | SyntaxKind::PropertyChangedCallback => {
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
                break (element, "$model".into(), false);
            }
            SyntaxKind::Element => {
                // oops: missed it
                let element = syntax_nodes::Element::new(n)?;
                break (element, SmolStr::default(), false);
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

        let window = dc.element_at_position(&url, &lsp_types::Position::new(0, 30));
        assert_eq!(find_element_indent(&window.unwrap()), None);

        let vbox = dc.element_at_position(&url, &lsp_types::Position::new(1, 4));
        assert_eq!(find_element_indent(&vbox.unwrap()), Some("    ".to_string()));

        let label = dc.element_at_position(&url, &lsp_types::Position::new(2, 17));
        assert_eq!(find_element_indent(&label.unwrap()), Some("        ".to_string()));
    }

    #[test]
    fn test_map_position() {
        let text = r#"// 🔥 Test 🎆
component MainWindow inherits Window {
    VerticalBox {
        label := Text { text: "te🦥xt"; }
    }
}"#
        .to_string();
        let (dc, url, _) = loaded_document_cache(text.clone());
        let doc = dc.get_document(&url).unwrap();
        let source = doc.node.as_ref().unwrap().source_file.clone();
        let mut offset = TextSize::new(0);
        let mut line = 0_usize;
        let mut pos = 0_usize;
        for c in text.chars() {
            let original_offset = offset;
            let mapped = text_size_to_lsp_position(&source, u32::from(original_offset).into());
            eprintln!(
                "c: {c} <offset: {offset:?}> => {line}:{pos} => mapped {}:{}",
                mapped.line, mapped.character
            );
            assert_eq!(mapped.line, (line as u32));
            assert_eq!(mapped.character, (pos as u32));
            let unmapped = lsp_position_to_text_size(&source, mapped);
            assert_eq!(unmapped, original_offset);
            offset = offset.checked_add((c.len_utf8() as u32).into()).unwrap();
            match c {
                '\n' => {
                    line += 1;
                    pos = 0
                }
                c => {
                    pos += c.len_utf8();
                }
            }
        }
    }
}
