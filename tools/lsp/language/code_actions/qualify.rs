// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore rfind

use super::Context;
use crate::util;

use i_slint_compiler::lookup::LookupObject;
use i_slint_compiler::parser::SyntaxKind;
use lsp_types::{CodeActionKind, CodeActionOrCommand, TextEdit};

pub(super) fn add(ctx: &Context, result: &mut Vec<CodeActionOrCommand>) {
    let is_bare_identifier = ctx.token.kind() == SyntaxKind::Identifier
        && ctx.node.kind() == SyntaxKind::QualifiedName
        && ctx.node.parent().map(|n| n.kind()) == Some(SyntaxKind::Expression)
        && ctx.node.children_with_tokens().filter(|n| n.kind() == SyntaxKind::Identifier).count()
            == 1;
    if !is_bare_identifier {
        return;
    }
    let suggestions =
        util::with_lookup_ctx(ctx.document_cache, ctx.node.clone(), None, |lookup_ctx| {
            let name = i_slint_compiler::parser::normalize_identifier(ctx.token.text());
            if i_slint_compiler::lookup::global_lookup().lookup(lookup_ctx, &name).is_none() {
                i_slint_compiler::lookup::enum_or_color_suggestions(lookup_ctx, ctx.token.text())
            } else {
                Vec::new()
            }
        })
        .unwrap_or_default();
    let range = util::text_range_to_lsp_range(
        &ctx.token.source_file,
        ctx.token.text_range(),
        ctx.document_cache.format,
    );
    for suggestion in suggestions {
        ctx.push_edit(
            result,
            format!("Qualify as '{suggestion}'"),
            Some(CodeActionKind::QUICKFIX),
            vec![TextEdit::new(range, suggestion.to_string())],
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::language::code_actions::test::CodeActionRequest;

    #[test]
    fn test_qualify_named_color() {
        CodeActionRequest::at("export component TestCase {\n    init => { debug(🔺red); }\n}\n")
            .load()
            .assert(
                "Qualify as 'Colors.red'",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                "export component TestCase {\n    init => { debug(Colors.red); }\n}\n",
            )
            .compiles();
    }

    #[test]
    fn test_no_qualify_for_resolved_color() {
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
            ["Qualify as 'EA.shared-value'", "Qualify as 'EB.shared-value'"]
        );
        for qualified in ["EA.shared-value", "EB.shared-value"] {
            actions
                .assert(
                    &format!("Qualify as '{qualified}'"),
                    Some(lsp_types::CodeActionKind::QUICKFIX),
                    &SOURCE.replace("(shared_value)", &format!("({qualified})")),
                )
                .compiles();
        }
    }
}
