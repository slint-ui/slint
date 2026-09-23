// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code actions and completions for the cases of a `match` element.

use crate::editor_preview::{self, DocumentCache};
use crate::util;

#[cfg(target_arch = "wasm32")]
use crate::editor_preview::wasm_prelude::*;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::Type;
use i_slint_compiler::object_tree::{CaseValue, MatchSubjectDomain, missing_case_values};
use i_slint_compiler::parser::{
    SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize, syntax_nodes,
};
use lsp_types::{CodeActionOrCommand, CompletionItem, CompletionItemKind, TextEdit};

struct MatchCases {
    subject_type: Type,
    covered: Vec<CaseValue>,
    has_non_literal_case: bool,
}

fn analyze(
    document_cache: &DocumentCache,
    match_element: &syntax_nodes::MatchElement,
    skip: Option<TextRange>,
) -> Option<MatchCases> {
    let subject_node = match_element.child_node(SyntaxKind::Expression)?;
    util::with_lookup_ctx(document_cache, subject_node.clone(), None, |ctx| {
        let subject_type = Expression::from_expression_node(subject_node.into(), ctx).ty();
        ctx.property_type = subject_type.clone();
        ctx.expected_type = subject_type.clone();

        let mut covered = Vec::new();
        let mut has_non_literal_case = false;
        for case in match_element.MatchCase() {
            let Some(case_node) = case.child_node(SyntaxKind::Expression) else {
                continue;
            };
            if skip == Some(case_node.text_range()) {
                continue;
            }
            let expression = Expression::from_expression_node(case_node.into(), ctx);
            match CaseValue::new(&expression) {
                Some(value) => covered.push(value),
                None if matches!(expression, Expression::Invalid) => {}
                None => has_non_literal_case = true,
            }
        }
        MatchCases { subject_type, covered, has_non_literal_case }
    })
}

fn enclosing_match_element(token: &SyntaxToken) -> Option<syntax_nodes::MatchElement> {
    let node = token.parent_ancestors().find(|node| {
        matches!(
            node.kind(),
            SyntaxKind::MatchCase | SyntaxKind::WildcardMatchCase | SyntaxKind::MatchElement
        )
    })?;
    syntax_nodes::MatchElement::new(node)
}

pub fn add_code_actions(
    document_cache: &DocumentCache,
    token: &SyntaxToken,
    result: &mut Vec<CodeActionOrCommand>,
) -> Option<()> {
    if !document_cache.compiler_configuration().enable_experimental {
        return None;
    }
    let match_element = enclosing_match_element(token)?;
    if match_element.WildcardMatchCase().is_some() {
        return None;
    }

    let open = match_element.child_token(SyntaxKind::LBrace)?;

    let cases = analyze(document_cache, &match_element, None)?;
    let (title, values) = match MatchSubjectDomain::of(&cases.subject_type) {
        MatchSubjectDomain::Unknown => return None,
        MatchSubjectDomain::Unbounded => ("Add '*' case", vec!["*".to_string()]),
        MatchSubjectDomain::Exhaustive(domain) => {
            if cases.has_non_literal_case {
                return None;
            }
            let missing = missing_case_values(&domain, &cases.covered);
            if missing.is_empty() {
                return None;
            }
            ("Add missing cases", missing.iter().map(|value| value.to_string()).collect())
        }
    };

    let match_indent = util::find_indent(&match_element).unwrap_or_default();
    let last_case = match_element.MatchCase().last();
    let case_indent = last_case
        .as_ref()
        .and_then(|case| own_line_indent(case))
        .unwrap_or_else(|| format!("{match_indent}    "));
    let anchor = anchor_after_trailing_comment(
        &last_case.as_ref().and_then(|case| case.last_token()).unwrap_or(open),
    );

    let mut text = String::new();
    for value in values {
        text.push_str(&format!("\n{case_indent}{value}: {{ }}"));
    }

    let source = token.source_file.source()?;
    let close = match_element.child_token(SyntaxKind::RBrace).filter(|close| {
        source
            .get(usize::from(anchor)..usize::from(close.text_range().start()))
            .is_some_and(|gap| !gap.contains('\n') && gap.trim().is_empty())
    });
    let end = match close {
        Some(close) => {
            text.push_str(&format!("\n{match_indent}"));
            close.text_range().start()
        }
        None => anchor,
    };

    let range = util::text_range_to_lsp_range(
        &token.source_file,
        TextRange::new(anchor, end),
        document_cache.format,
    );
    result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
        title: title.into(),
        kind: Some(lsp_types::CodeActionKind::QUICKFIX),
        edit: editor_preview::editing::create_workspace_edit_from_path(
            document_cache,
            token.source_file.path(),
            vec![TextEdit::new(range, text)],
        ),
        ..Default::default()
    }));
    Some(())
}

fn anchor_after_trailing_comment(after: &SyntaxToken) -> TextSize {
    let mut anchor = after.text_range().end();
    let mut current = after.next_token();
    while let Some(token) = current {
        match token.kind() {
            SyntaxKind::Whitespace if !token.text().contains('\n') => {}
            SyntaxKind::Comment => anchor = token.text_range().end(),
            _ => break,
        }
        current = token.next_token();
    }
    anchor
}

fn own_line_indent(node: &SyntaxNode) -> Option<String> {
    let previous = node.first_token()?.prev_token()?;
    (previous.kind() == SyntaxKind::Whitespace && previous.text().contains('\n'))
        .then(|| previous.text().rsplit('\n').next().unwrap_or_default().to_string())
}

pub fn case_value_position(
    token: &SyntaxToken,
    offset: TextSize,
) -> Option<syntax_nodes::MatchElement> {
    let literal = matches!(
        token.kind(),
        SyntaxKind::StringLiteral | SyntaxKind::NumberLiteral | SyntaxKind::ColorLiteral
    );
    if literal && token.text_range().contains(offset) && offset > token.text_range().start() {
        return None;
    }

    let node = token.parent();
    if let Some(match_element) = syntax_nodes::MatchElement::new(node.clone()) {
        let open = match_element.child_token(SyntaxKind::LBrace)?;
        let close = match_element.child_token(SyntaxKind::RBrace)?;
        let body = TextRange::new(open.text_range().end(), close.text_range().start());
        return body.contains_inclusive(offset).then_some(match_element);
    }

    let mut candidate = node;
    loop {
        match candidate.kind() {
            SyntaxKind::MatchCase => break,
            SyntaxKind::WildcardMatchCase | SyntaxKind::MatchElement => return None,
            _ => candidate = candidate.parent()?,
        }
    }
    let after_body = candidate
        .child_node(SyntaxKind::SubElement)
        .is_some_and(|body| offset >= body.text_range().end());
    if !after_body
        && let Some(colon) = candidate.child_token(SyntaxKind::Colon)
        && offset > colon.text_range().start()
    {
        return None;
    }
    syntax_nodes::MatchElement::new(candidate.parent()?)
}

pub fn case_value_completions(
    document_cache: &DocumentCache,
    match_element: &syntax_nodes::MatchElement,
    offset: TextSize,
) -> Option<Vec<CompletionItem>> {
    let skip = match_element
        .MatchCase()
        .filter_map(|case| case.child_node(SyntaxKind::Expression))
        .find(|value| value.text_range().contains_inclusive(offset))
        .map(|value| value.text_range());
    let cases = analyze(document_cache, match_element, skip)?;
    match MatchSubjectDomain::of(&cases.subject_type) {
        MatchSubjectDomain::Unknown => None,
        MatchSubjectDomain::Unbounded => {
            if match_element.WildcardMatchCase().is_some() {
                return None;
            }
            Some(vec![CompletionItem {
                kind: Some(CompletionItemKind::KEYWORD),
                ..CompletionItem::new_simple("*".to_string(), String::new())
            }])
        }
        MatchSubjectDomain::Exhaustive(domain) => {
            if match_element
                .WildcardMatchCase()
                .is_some_and(|wildcard| offset > wildcard.text_range().start())
            {
                return None;
            }
            let kind = match cases.subject_type {
                Type::Bool => CompletionItemKind::KEYWORD,
                _ => CompletionItemKind::ENUM_MEMBER,
            };
            Some(
                missing_case_values(&domain, &cases.covered)
                    .into_iter()
                    .map(|value| CompletionItem {
                        kind: Some(kind),
                        ..CompletionItem::new_simple(value.to_string(), String::new())
                    })
                    .collect(),
            )
        }
    }
}
