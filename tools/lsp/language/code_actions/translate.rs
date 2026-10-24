// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::Context;
use crate::util;

use i_slint_compiler::parser::SyntaxKind;
use lsp_types::{CodeActionOrCommand, TextEdit};

pub(super) fn add(ctx: &Context, result: &mut Vec<CodeActionOrCommand>) {
    if ctx.token.kind() != SyntaxKind::StringLiteral || ctx.node.kind() != SyntaxKind::Expression {
        return;
    }
    let range = util::text_range_to_lsp_range(
        &ctx.token.source_file,
        ctx.node.text_range(),
        ctx.document_cache.format,
    );
    let edits = vec![
        TextEdit::new(lsp_types::Range::new(range.start, range.start), "@tr(".into()),
        TextEdit::new(lsp_types::Range::new(range.end, range.end), ")".into()),
    ];
    ctx.push_edit(result, "Wrap in `@tr()`", None, edits);
}

#[cfg(test)]
mod tests {
    use crate::language::code_actions::test::CodeActionRequest;

    #[test]
    fn test_code_action_wrap_string_in_tr() {
        CodeActionRequest::at(r#"export component Test inherits Text { text: 🔺"Hello"; }"#)
            .load()
            .assert(
                "Wrap in `@tr()`",
                None,
                r#"export component Test inherits Text { text: @tr("Hello"); }"#,
            )
            .compiles();
    }
}
