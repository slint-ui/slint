// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code actions and completions for the cases of a `match` element.

// cSpell: ignore descr

use crate::editor_preview::{self, DocumentCache};
use crate::util;

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
    if !document_cache
        .compiler_configuration()
        .compiler_config
        .experimental_features
        .contains(i_slint_compiler::ExperimentalFeature::Match)
    {
        return None;
    }
    let match_element = enclosing_match_element(token)?;
    if match_element.WildcardMatchCase().is_some() {
        return None;
    }

    let open = match_element.child_token(SyntaxKind::LBrace)?;

    let cases = analyze(document_cache, &match_element, None)?;
    let (title, values) = match MatchSubjectDomain::of(&cases.subject_type) {
        MatchSubjectDomain::Unknown | MatchSubjectDomain::Unsupported => return None,
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
        MatchSubjectDomain::Unknown | MatchSubjectDomain::Unsupported => None,
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
    use crate::language::test::CodeActionRequest;
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
    fn match_case_values_unsupported_subject() {
        let results = get_completions_experimental(
            r#"export component Test {
    in-out property <image> picture;

    match picture { 🔺 }
}
"#,
        )
        .unwrap_or_default();
        assert!(!labels(&results).contains(&"*"));
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

    const NUMS: &str = "enum Nums { one, two, three }\n";

    #[test]
    fn test_code_actions_fill_missing_enum_cases() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{
        one: Rectangle {{ }}
    }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        one: Rectangle {{ }}
        two: {{ }}
        three: {{ }}
    }}
}}
"#
                ),
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_fill_missing_enum_cases_qualified() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{
        Nums.one: Rectangle {{ }}
    }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        Nums.one: Rectangle {{ }}
        two: {{ }}
        three: {{ }}
    }}
}}
"#
                ),
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_fill_missing_bool_case() {
        let actions = CodeActionRequest::at(
            r#"export component Test {
    in-out property <bool> flag: true;

    match 🔺flag {
        true: Rectangle { }
    }
}
"#,
        )
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                r#"export component Test {
    in-out property <bool> flag: true;

    match flag {
        true: Rectangle { }
        false: { }
    }
}
"#,
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_add_wildcard_case() {
        let actions = CodeActionRequest::at(
            r#"export component Test {
    in-out property <int> num: 0;

    match 🔺num {
        0: Rectangle { }
    }
}
"#,
        )
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add '*' case"]);
        actions
            .assert(
                "Add '*' case",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                r#"export component Test {
    in-out property <int> num: 0;

    match num {
        0: Rectangle { }
        *: { }
    }
}
"#,
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_no_wildcard_for_unsupported_subject() {
        CodeActionRequest::at(
            r#"export component Test {
    in-out property <image> picture;

    match 🔺picture {
        @image-url("x.png"): Rectangle { }
    }
}
"#,
        )
        .experimental()
        .load()
        .assert_empty();
    }

    #[test]
    fn test_code_actions_no_fill_for_exhaustive_match() {
        let source = format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        one: Rectangle {{ }}
        two: Rectangle {{ }}
        three: Rectangle {{ }}
    }}

    match num {{
        one: Rectangle {{ }}
        *: Rectangle {{ }}
    }}
}}
"#
        );
        let subject = "match ".len();
        CodeActionRequest::at_offset(&source, source.find("match num").unwrap() + subject)
            .experimental()
            .load()
            .assert_empty();
        CodeActionRequest::at_offset(&source, source.rfind("match num").unwrap() + subject)
            .experimental()
            .load()
            .assert_empty();
    }

    #[test]
    fn test_code_actions_match_trigger_positions() {
        let source = format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        one: Rectangle {{
            match num {{
                two: Rectangle {{ }}
            }}
        }}
    }}
}}
"#
        );
        let outer = source.find("match num").unwrap();
        for offset in [
            outer + "m".len(),
            outer + "match ".len(),
            source.rfind("    }\n}").unwrap() + "    ".len(),
        ] {
            let actions = CodeActionRequest::at_offset(&source, offset).experimental().load();
            assert_eq!(actions.titles(), ["Add missing cases"], "offset {offset}");
        }
        CodeActionRequest::at_offset(&source, source.find("Rectangle").unwrap())
            .experimental()
            .load()
            .assert_empty();
        let actions = CodeActionRequest::at_offset(
            &source,
            source.rfind("match num").unwrap() + "match ".len(),
        )
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        one: Rectangle {{
            match num {{
                two: Rectangle {{ }}
                one: {{ }}
                three: {{ }}
            }}
        }}
    }}
}}
"#
                ),
            )
            .skip_compile("the outer match stays non-exhaustive");
    }

    #[test]
    fn test_code_actions_fill_empty_match_body() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{ }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        one: {{ }}
        two: {{ }}
        three: {{ }}
    }}
}}
"#
                ),
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_fill_one_line_match() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{ one: Rectangle {{ }} }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{ one: Rectangle {{ }}
        two: {{ }}
        three: {{ }}
    }}
}}
"#
                ),
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_fill_preserves_trailing_comment() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{ /* keep */ }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{ /* keep */
        one: {{ }}
        two: {{ }}
        three: {{ }}
    }}
}}
"#
                ),
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_fill_trailing_comment_without_cases() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{ // start
    }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{ // start
        one: {{ }}
        two: {{ }}
        three: {{ }}
    }}
}}
"#
                ),
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_fill_trailing_comment_on_last_case() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{
        one: Rectangle {{ }} // keep
    }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        one: Rectangle {{ }} // keep
        two: {{ }}
        three: {{ }}
    }}
}}
"#
                ),
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_fill_several_trailing_comments() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{
        one: Rectangle {{ }} /* a */ /* b */
    }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        one: Rectangle {{ }} /* a */ /* b */
        two: {{ }}
        three: {{ }}
    }}
}}
"#
                ),
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_fill_keeps_comment_on_its_own_line() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{
        one: Rectangle {{ }}
        // keep
    }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        one: Rectangle {{ }}
        two: {{ }}
        three: {{ }}
        // keep
    }}
}}
"#
                ),
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_fill_tab_indented() {
        let actions = CodeActionRequest::at(&format!(
                "{NUMS}\nexport component Test {{\n\tin-out property <Nums> num: one;\n\n\tma🔺tch num {{\n\t\tone: Rectangle {{ }}\n\t}}\n}}\n"
            ))
            .experimental().load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions.assert("Add missing cases", Some(lsp_types::CodeActionKind::QUICKFIX), &format!(
                "{NUMS}\nexport component Test {{\n\tin-out property <Nums> num: one;\n\n\tmatch num {{\n\t\tone: Rectangle {{ }}\n\t\ttwo: {{ }}\n\t\tthree: {{ }}\n\t}}\n}}\n"
            )).compiles();
    }

    #[test]
    fn test_code_actions_fill_with_duplicate_case() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{
        one: Rectangle {{ }}
        one: Rectangle {{ }}
    }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        one: Rectangle {{ }}
        one: Rectangle {{ }}
        two: {{ }}
        three: {{ }}
    }}
}}
"#
                ),
            )
            .skip_compile("the duplicate case stays");
    }

    #[test]
    fn test_code_actions_match_inside_for() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
struct Item {{ num: Nums }}

export component Test {{
    in-out property <[Item]> items;

    for item in items: Rectangle {{
        match i🔺tem.num {{
            one: Rectangle {{ }}
        }}
    }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
struct Item {{ num: Nums }}

export component Test {{
    in-out property <[Item]> items;

    for item in items: Rectangle {{
        match item.num {{
            one: Rectangle {{ }}
            two: {{ }}
            three: {{ }}
        }}
    }}
}}
"#
                ),
            )
            .compiles();
    }

    #[test]
    fn test_code_actions_fill_with_typo_case() {
        let actions = CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{
        onee: Rectangle {{ }} // cspell:disable-line
    }}
}}
"#
        ))
        .experimental()
        .load();
        assert_eq!(actions.titles(), ["Add missing cases"]);
        actions
            .assert(
                "Add missing cases",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                &format!(
                    r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match num {{
        onee: Rectangle {{ }} // cspell:disable-line
        one: {{ }}
        two: {{ }}
        three: {{ }}
    }}
}}
"#
                ),
            )
            .skip_compile("the misspelled case stays");
    }

    #[test]
    fn test_code_actions_no_fill_for_non_literal_case() {
        CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;
    in-out property <Nums> other: two;

    match 🔺num {{
        other: Rectangle {{ }}
    }}
}}
"#
        ))
        .experimental()
        .load()
        .assert_empty();
    }

    #[test]
    fn test_code_actions_no_fill_for_missing_brace() {
        CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num
        one: Rectangle {{ }}
    }}
}}
"#
        ))
        .experimental()
        .load()
        .assert_empty();
    }

    #[test]
    fn test_code_actions_fill_requires_experimental() {
        CodeActionRequest::at(&format!(
            r#"{NUMS}
export component Test {{
    in-out property <Nums> num: one;

    match 🔺num {{
        one: Rectangle {{ }}
    }}
}}
"#
        ))
        .load()
        .assert_empty();
    }
}
