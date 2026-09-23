// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Completions for the cases of a `match` element.

use crate::editor_preview::DocumentCache;
use crate::util;

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

#[cfg(test)]
mod tests {
    use crate::language::completion::tests::{get_completions, get_completions_experimental};
    use lsp_types::{CompletionItem, CompletionItemKind};

    fn match_body(body: &str) -> String {
        format!(
            r#"enum Nums {{ one, two, three }}
export component Test {{
    in-out property <Nums> num: one;

    match num {{{body}}}
}}
"#
        )
    }

    fn labels(results: &[CompletionItem]) -> Vec<&str> {
        results.iter().map(|completion| completion.label.as_str()).collect()
    }

    #[test]
    fn match_case_values_enum() {
        let results = get_completions_experimental(&match_body(" 🔺 ")).unwrap();
        assert_eq!(labels(&results), ["one", "two", "three"]);
        assert_eq!(results[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    }

    #[test]
    fn match_case_values_exclude_covered() {
        let results = get_completions_experimental(&match_body(
            "\n        one: Rectangle { }\n        🔺\n    ",
        ))
        .unwrap();
        assert_eq!(labels(&results), ["two", "three"]);
    }

    #[test]
    fn match_case_values_adjacent_to_previous_case() {
        let results = get_completions_experimental(&match_body(" one: Rectangle { }🔺 ")).unwrap();
        assert_eq!(labels(&results), ["two", "three"]);
    }

    #[test]
    fn match_case_values_after_wildcard() {
        let results =
            get_completions_experimental(&match_body(" *: Rectangle { } 🔺 ")).unwrap_or_default();
        assert!(labels(&results).is_empty());
    }

    #[test]
    fn match_case_values_before_wildcard() {
        let results = get_completions_experimental(&match_body(" 🔺 *: Rectangle { } ")).unwrap();
        assert_eq!(labels(&results), ["one", "two", "three"]);
    }

    #[test]
    fn match_case_values_adjacent_to_wildcard() {
        // The cursor touches the '*' token, so it counts as inside the wildcard case
        let results =
            get_completions_experimental(&match_body(" 🔺*: Rectangle { } ")).unwrap_or_default();
        assert!(labels(&results).is_empty());
    }

    #[test]
    fn match_case_values_bool() {
        let results = get_completions_experimental(
            r#"export component Test {
    in-out property <bool> flag: true;

    match flag { true: Rectangle { } 🔺 }
}
"#,
        )
        .unwrap();
        assert_eq!(labels(&results), ["false"]);
        assert_eq!(results[0].kind, Some(CompletionItemKind::KEYWORD));
    }

    #[test]
    fn match_case_values_unbounded_subject() {
        let results = get_completions_experimental(
            r#"export component Test {
    in-out property <int> num;

    match num { 🔺 }
}
"#,
        )
        .unwrap();
        assert_eq!(labels(&results), ["*"]);
        assert_eq!(results[0].kind, Some(CompletionItemKind::KEYWORD));
    }

    #[test]
    fn match_case_values_partial_identifier() {
        let results = get_completions_experimental(&match_body(" t🔺 ")).unwrap();
        assert_eq!(labels(&results), ["one", "two", "three"]);
    }

    fn typed_match_body(property_type: &str, body: &str) -> String {
        format!(
            r#"export component Test {{
    in-out property <{property_type}> subject;

    match subject {{{body}}}
}}
"#
        )
    }

    #[test]
    fn match_case_values_inside_string_literal() {
        let results =
            get_completions_experimental(&typed_match_body("string", r#" "a🔺": Rectangle { } "#))
                .unwrap_or_default();
        assert!(labels(&results).is_empty());
    }

    #[test]
    fn match_case_values_after_string_literal() {
        // The offset sits on the token boundary, which is a position for a new case
        let results =
            get_completions_experimental(&typed_match_body("string", r#" "a"🔺 "#)).unwrap();
        assert_eq!(labels(&results), ["*"]);
    }

    #[test]
    fn match_case_values_inside_string_literal_with_enum_subject() {
        let results = get_completions_experimental(&match_body(r#" "o🔺" "#)).unwrap_or_default();
        assert!(labels(&results).is_empty());
    }

    #[test]
    fn match_case_values_inside_number_literal() {
        let results =
            get_completions_experimental(&typed_match_body("int", " 1🔺2 ")).unwrap_or_default();
        assert!(!labels(&results).contains(&"*"));
    }

    #[test]
    fn match_case_values_inside_color_literal() {
        let results = get_completions_experimental(&typed_match_body("color", " #f0🔺0 "))
            .unwrap_or_default();
        assert!(!labels(&results).contains(&"*"));
    }

    #[test]
    fn match_case_values_not_in_case_body() {
        let results = get_completions_experimental(&match_body(" one: Rectangle { 🔺 } ")).unwrap();
        assert!(results.iter().any(|completion| completion.label == "background"));
        assert!(!results.iter().any(|completion| completion.label == "two"));
    }

    #[test]
    fn match_keyword_in_element() {
        let results = get_completions_experimental(
            r#"export component Test {
    Rectangle {
        🔺
    }
}
"#,
        )
        .unwrap();
        let completion = results.iter().find(|completion| completion.label == "match").unwrap();
        assert_eq!(
            completion.insert_text.as_deref(),
            Some("match $1 {\n    $2: ${3:Rectangle} {\n        $0\n    }\n}")
        );
    }

    #[test]
    fn match_keyword_requires_experimental() {
        let results = get_completions(
            r#"export component Test {
    Rectangle {
        🔺
    }
}
"#,
        )
        .unwrap();
        assert!(!results.iter().any(|completion| completion.label == "match"));
    }

    #[test]
    fn match_case_values_requires_experimental() {
        let results = get_completions(&match_body(" 🔺 ")).unwrap_or_default();
        for value in ["one", "two", "three"] {
            assert!(!labels(&results).contains(&value));
        }
    }
}
