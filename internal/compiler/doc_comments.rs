// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Extract `///` doc comments from the syntax tree.

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::langtype::{BuiltinElement, ElementDocEntry};
use crate::parser::{SyntaxKind, SyntaxNode, identifier_text, syntax_nodes};

/// Strip a doc-comment prefix (`///` or `//!`) from a line.
/// Returns the content after the prefix if the line matches exactly
/// `prefix` or `prefix` followed by a space and content.
/// Rejects lines like `////` or `//!!`.
fn strip_doc_prefix<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(prefix)?;
    match rest.strip_prefix(' ') {
        Some(content) => Some(content),
        None if rest.is_empty() => Some(""),
        None => None,
    }
}

/// Walk backwards across sibling tokens/nodes collecting consecutive
/// `///` doc comment lines immediately before `anchor`. Returns the
/// concatenated text with the `/// ` prefix stripped, or `None` if
/// no doc comment was present.
fn collect_before(anchor: &SyntaxNode) -> Option<String> {
    let mut lines = Vec::new();
    let mut cursor = anchor.node.prev_sibling_or_token();
    while let Some(cur) = cursor {
        match cur.kind() {
            SyntaxKind::Whitespace => {}
            SyntaxKind::Comment => {
                let text = cur.as_token().unwrap().text();
                if let Some(content) = strip_doc_prefix(text, "///") {
                    lines.push(content.to_string());
                } else if text.starts_with("//") {
                    // Skip regular comments and //-annotations.
                } else {
                    break;
                }
            }
            SyntaxKind::ExportsList => {
                // Doc comments may sit inside a preceding `export { ... }` list.
                if let Some(list) = cur.as_node() {
                    let mut last = list.last_child_or_token();
                    while let Some(child) = last {
                        match child.kind() {
                            SyntaxKind::Whitespace => {}
                            SyntaxKind::Comment => {
                                let t = child.as_token().unwrap().text();
                                if let Some(content) = strip_doc_prefix(t, "///") {
                                    lines.push(content.to_string());
                                } else if t.starts_with("//") {
                                    // skip
                                } else {
                                    break;
                                }
                            }
                            _ => break,
                        }
                        last = child.prev_sibling_or_token();
                    }
                }
                break;
            }
            _ => break,
        }
        cursor = cur.prev_sibling_or_token();
    }
    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    Some(lines.join("\n"))
}

/// Extract the `///` doc comment before a syntax node. Also checks
/// above the enclosing `ExportsList` when the node is inside one.
pub(crate) fn doc_comment(anchor: &SyntaxNode) -> Option<String> {
    if let Some(doc) = collect_before(anchor) {
        return Some(doc);
    }
    if let Some(parent) = anchor.parent()
        && parent.kind() == SyntaxKind::ExportsList
    {
        return collect_before(&parent);
    }
    None
}

/// Extract the `///` description before the component and the ordered
/// body entries (`//!` text and member references) from inside it.
/// The description is included as the first `Text` entry.
pub(crate) fn element_doc_entries(
    component: &SyntaxNode,
    element: &syntax_nodes::Element,
    diag: &mut BuildDiagnostics,
) -> Vec<ElementDocEntry> {
    let description = doc_comment(component).unwrap_or_default();

    let mut entries = vec![ElementDocEntry::Text(description)];
    let mut section_lines: Vec<String> = Vec::new();
    let flush_section = |lines: &mut Vec<String>, entries: &mut Vec<ElementDocEntry>| {
        if !lines.is_empty() {
            entries.push(ElementDocEntry::Text(lines.join("\n")));
            lines.clear();
        }
    };

    let mut doc_comment_span = None;
    for child in element.children_with_tokens() {
        match child.kind() {
            SyntaxKind::Whitespace => {}
            SyntaxKind::Comment => {
                if let Some(t) = child.as_token() {
                    let text = t.text();
                    if strip_doc_prefix(text, "///").is_some() {
                        doc_comment_span = Some(child.to_source_location());
                    } else if let Some(content) = strip_doc_prefix(text, "//!") {
                        if let Some(span) = doc_comment_span.take() {
                            diag.push_warning_with_span("`///` doc comment not attached to a declaration".into(), span);
                        }
                        section_lines.push(content.to_string());
                    }
                }
            }
            SyntaxKind::PropertyDeclaration => {
                doc_comment_span = None;
                let p = syntax_nodes::PropertyDeclaration::from(child.into_node().unwrap());
                if p.TwoWayBinding().is_some() {
                    continue;
                }
                flush_section(&mut section_lines, &mut entries);
                let name = identifier_text(&p.DeclaredIdentifier()).unwrap();
                entries.push(ElementDocEntry::Member(name));
            }
            SyntaxKind::CallbackDeclaration => {
                doc_comment_span = None;
                let cb = syntax_nodes::CallbackDeclaration::from(child.into_node().unwrap());
                if cb.TwoWayBinding().is_some() {
                    continue;
                }
                flush_section(&mut section_lines, &mut entries);
                let name = identifier_text(&cb.DeclaredIdentifier()).unwrap();
                entries.push(ElementDocEntry::Member(name));
            }
            SyntaxKind::Function => {
                doc_comment_span = None;
                let f = syntax_nodes::Function::from(child.into_node().unwrap());
                flush_section(&mut section_lines, &mut entries);
                let name = identifier_text(&f.DeclaredIdentifier()).unwrap();
                entries.push(ElementDocEntry::Member(name));
            }
            _ => {
                if let Some(span) = doc_comment_span.take() {
                    diag.push_warning_with_span("`///` doc comment not attached to a declaration".into(), span);
                }
            }
        }
    }
    if let Some(span) = doc_comment_span.take() {
        diag.push_warning_with_span("`///` doc comment not attached to a declaration".into(), span);
    }
    flush_section(&mut section_lines, &mut entries);
    entries
}

/// Assemble the final doc entries for an element, prepending inherited
/// parent entries after the description.
pub(crate) fn assemble(
    mut entries: Vec<ElementDocEntry>,
    parent: Option<&BuiltinElement>,
) -> Vec<ElementDocEntry> {
    let skip_inherited = matches!(entries.first(), Some(ElementDocEntry::Text(desc)) if desc.contains("\\skip_inherited"));

    if !skip_inherited && let Some(parent) = parent {
        // Splice inherited parent body (everything after parent's description)
        // right after our own description (entries[0]).
        entries.splice(1..1, parent.docs[1..].iter().cloned());
    }
    entries
}
