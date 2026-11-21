// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::Context;
use lsp_types::{CodeActionKind, CodeActionOrCommand};

/// The compiler's suggestions for the `diagnostics` the client sent with the request.
pub(super) fn add(
    ctx: &Context,
    diagnostics: &[lsp_types::Diagnostic],
    result: &mut Vec<CodeActionOrCommand>,
) {
    for diagnostic in diagnostics {
        let suggestions = i_slint_live_preview::protocol::lsp_diagnostic_suggestions(diagnostic);
        let is_preferred = suggestions.len() == 1;
        for edit in suggestions {
            let title = format!("Replace with '{}'", edit.new_text);
            result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                diagnostics: Some(vec![diagnostic.clone()]),
                is_preferred: is_preferred.then_some(true),
                ..ctx.edit_action(title, Some(CodeActionKind::QUICKFIX), vec![edit])
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::language::code_actions::test::CodeActionRequest;
    use lsp_types::CodeActionKind;

    #[test]
    fn test_qualify_named_color() {
        CodeActionRequest::at("export component TestCase {\n    init => { debug(🔺red); }\n}\n")
            .load()
            .assert(
                "Replace with 'Colors.red'",
                Some(CodeActionKind::QUICKFIX),
                "export component TestCase {\n    init => { debug(Colors.red); }\n}\n",
            )
            .compiles();
    }

    #[test]
    fn test_no_suggestion_for_resolved_color() {
        CodeActionRequest::at("export component TestCase {\n    property <color> c: 🔺red;\n}\n")
            .load()
            .assert_empty();
    }

    #[test]
    fn test_qualify_value_shared_by_enums() {
        const SOURCE: &str = r#"enum EA { shared_value }
enum EB { shared_value }
export component TestCase {
    init => { debug(shared_value); }
}
"#;
        let actions =
            CodeActionRequest::at_offset(SOURCE, SOURCE.rfind("shared_value").unwrap()).load();
        assert_eq!(
            actions.titles(),
            ["Replace with 'EA.shared-value'", "Replace with 'EB.shared-value'"]
        );
        for qualified in ["EA.shared-value", "EB.shared-value"] {
            actions
                .assert(
                    &format!("Replace with '{qualified}'"),
                    Some(CodeActionKind::QUICKFIX),
                    &SOURCE.replace("(shared_value)", &format!("({qualified})")),
                )
                .compiles();
        }
    }

    #[test]
    fn test_misspelled_property() {
        CodeActionRequest::at("export component TestCase {\n    🔺with: 10px;\n}\n")
            .load()
            .assert(
                "Replace with 'width'",
                Some(CodeActionKind::QUICKFIX),
                "export component TestCase {\n    width: 10px;\n}\n",
            )
            .compiles();
    }

    #[test]
    fn test_misspelled_enum_value() {
        CodeActionRequest::at(
            "export component TestCase {\n    Text { horizontal-alignment: TextHorizontalAlignment.🔺centre; }\n}\n",
        )
        .load()
        .assert(
            "Replace with 'center'",
            Some(CodeActionKind::QUICKFIX),
            "export component TestCase {\n    Text { horizontal-alignment: TextHorizontalAlignment.center; }\n}\n",
        )
        .compiles();
    }

    #[test]
    fn test_missing_unit() {
        CodeActionRequest::at("export component TestCase {\n    width: 🔺50;\n}\n")
            .load()
            .assert(
                "Replace with '50px'",
                Some(CodeActionKind::QUICKFIX),
                "export component TestCase {\n    width: 50px;\n}\n",
            )
            .compiles();
        CodeActionRequest::at(
            "export component TestCase {\n    animate width { duration: 🔺1.5; }\n}\n",
        )
        .load()
        .assert(
            "Replace with '1.5ms'",
            Some(CodeActionKind::QUICKFIX),
            "export component TestCase {\n    animate width { duration: 1.5ms; }\n}\n",
        )
        .compiles();
    }

    #[test]
    fn test_unqualified_self_property() {
        CodeActionRequest::at("export component TestCase inherits Text {\n    in property <string> t: 🔺text;\n}\n")
            .load()
            .assert(
                "Replace with 'self.text'",
                Some(CodeActionKind::QUICKFIX),
                "export component TestCase inherits Text {\n    in property <string> t: self.text;\n}\n",
            )
            .compiles();
    }
}
