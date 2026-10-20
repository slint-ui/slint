// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::Context;
use crate::language::completion::match_element::analyze;
use crate::util;

use i_slint_compiler::object_tree::{MatchSubjectDomain, missing_case_values};
use i_slint_compiler::parser::{
    SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize, syntax_nodes,
};
use lsp_types::{CodeActionKind, CodeActionOrCommand, TextEdit};

pub(super) fn add(ctx: &Context, result: &mut Vec<CodeActionOrCommand>) {
    if let Some((title, edit)) = case_insertion(ctx) {
        ctx.push_edit(result, title, Some(CodeActionKind::QUICKFIX), vec![edit]);
    }
}

fn case_insertion(ctx: &Context) -> Option<(&'static str, TextEdit)> {
    let document_cache = ctx.document_cache;
    let token = &ctx.token;
    if !document_cache.compiler_configuration().compiler_config.enable_experimental {
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
    Some((title, TextEdit::new(range, text)))
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

#[cfg(test)]
mod tests {
    use crate::language::code_actions::test::CodeActionRequest;

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
