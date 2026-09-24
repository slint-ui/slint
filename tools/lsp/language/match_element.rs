// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Completions for the cases of a `match` element.

use crate::editor_preview::DocumentCache;
use crate::util;

#[cfg(target_arch = "wasm32")]
use crate::editor_preview::wasm_prelude::*;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::Type;
use i_slint_compiler::object_tree::{CaseValue, MatchSubjectDomain, missing_case_values};
use i_slint_compiler::parser::{SyntaxKind, SyntaxToken, TextRange, TextSize, syntax_nodes};
use lsp_types::{CompletionItem, CompletionItemKind};

struct MatchCases {
    subject_type: Type,
    covered: Vec<CaseValue>,
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
        for case in match_element.MatchCase() {
            let Some(case_node) = case.child_node(SyntaxKind::Expression) else {
                continue;
            };
            if skip == Some(case_node.text_range()) {
                continue;
            }
            let expression = Expression::from_expression_node(case_node.into(), ctx);
            if let Some(value) = CaseValue::new(&expression) {
                covered.push(value);
            }
        }
        MatchCases { subject_type, covered }
    })
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
