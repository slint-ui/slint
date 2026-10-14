// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::Context;
use crate::util;

use i_slint_compiler::object_tree::QualifiedTypeName;
use i_slint_compiler::parser::{SyntaxKind, syntax_nodes};
use lsp_types::{CodeActionKind, CodeActionOrCommand, TextEdit};

pub(super) fn add(ctx: &Context, result: &mut Vec<CodeActionOrCommand>) {
    if ctx.token.kind() != SyntaxKind::Identifier {
        return;
    }
    let Some(implement_statement) = (match ctx.node.kind() {
        SyntaxKind::ImplementStatement => syntax_nodes::ImplementStatement::new(ctx.node.clone()),
        SyntaxKind::QualifiedName | SyntaxKind::DeclaredIdentifier => {
            ctx.node.parent().and_then(syntax_nodes::ImplementStatement::new)
        }
        _ => None,
    }) else {
        return;
    };
    let statement_start = util::text_size_to_lsp_position(
        &ctx.token.source_file,
        implement_statement.text_range().start(),
        ctx.document_cache.format,
    );
    let Some(element) = ctx.document_cache.element_at_position(&ctx.uri, &statement_start) else {
        return;
    };
    let Some(missing_entry) =
        i_slint_compiler::object_tree::missing_interface_members(&element.element)
            .into_iter()
            .find(|entry| entry.statement.text_range() == implement_statement.text_range())
    else {
        return;
    };
    let indent = util::find_indent(&implement_statement).unwrap_or_default();
    let insertion: String = missing_entry
        .declarations
        .iter()
        .map(|declaration| format!("\n{indent}{declaration}"))
        .collect();
    let insert_position = util::text_size_to_lsp_position(
        &ctx.token.source_file,
        implement_statement.text_range().end(),
        ctx.document_cache.format,
    );
    let interface_name = QualifiedTypeName::from_node(implement_statement.QualifiedName());
    ctx.push_edit(
        result,
        format!("Add the missing declarations for '{interface_name}'"),
        Some(CodeActionKind::QUICKFIX),
        vec![TextEdit::new(lsp_types::Range::new(insert_position, insert_position), insertion)],
    );
}

#[cfg(test)]
mod tests {
    use crate::language::code_actions::test::CodeActionRequest;

    const SIGNATURE_INTERFACE: &str = r#"interface Signature {
    in property <bool> checkable;
    in property <string> text;
    callback clicked();
    public function add(a: int, b: int) -> int;
}

"#;

    #[test]
    fn test_implement_missing_members_inserts_all_declarations() {
        CodeActionRequest::at(&format!(
            "{SIGNATURE_INTERFACE}export component TestCase {{\n    implement 🔺Signature <=> self;\n}}\n"
        ))
        .experimental().load()
        .assert(
            "Add the missing declarations for 'Signature'",
            Some(lsp_types::CodeActionKind::QUICKFIX),
            &format!(
                r#"{SIGNATURE_INTERFACE}export component TestCase {{
    implement Signature <=> self;
    public function add(a: int, b: int) -> int {{ }}
    in property <bool> checkable;
    callback clicked();
    in property <string> text;
}}
"#
            ),
        ).skip_compile("the function body stays empty");
    }

    #[test]
    fn test_implement_missing_members_includes_inherited_members() {
        CodeActionRequest::at(
            r#"interface A {
    out property <int> speed;
}
interface B inherits A {
    out property <float> direction;
}
export component TestCase {
    implement 🔺B <=> self;
}
"#,
        )
        .experimental()
        .load()
        .assert(
            "Add the missing declarations for 'B'",
            Some(lsp_types::CodeActionKind::QUICKFIX),
            r#"interface A {
    out property <int> speed;
}
interface B inherits A {
    out property <float> direction;
}
export component TestCase {
    implement B <=> self;
    out property <float> direction;
    out property <int> speed;
}
"#,
        )
        .compiles();
    }

    #[test]
    fn test_implement_missing_members_offers_nothing_when_satisfied() {
        CodeActionRequest::at(&format!(
            r#"{SIGNATURE_INTERFACE}export component TestCase {{
    implement 🔺Signature <=> self;

    in property <bool> checkable;
    in property <string> text;
    callback clicked();
    public function add(a: int, b: int) -> int {{ 0 }}
}}
"#
        ))
        .experimental()
        .load()
        .assert_empty();
    }

    #[test]
    fn test_implement_missing_members_skips_members_with_wrong_type() {
        CodeActionRequest::at(&format!(
            "{SIGNATURE_INTERFACE}export component TestCase {{\n    implement 🔺Signature <=> self;\n\n    in property <int> checkable;\n}}\n"
        ))
        .experimental().load()
        .assert(
            "Add the missing declarations for 'Signature'",
            Some(lsp_types::CodeActionKind::QUICKFIX),
            &format!(
                r#"{SIGNATURE_INTERFACE}export component TestCase {{
    implement Signature <=> self;
    public function add(a: int, b: int) -> int {{ }}
    callback clicked();
    in property <string> text;

    in property <int> checkable;
}}
"#
            ),
        ).skip_compile("the wrongly typed member stays");
    }

    #[test]
    fn test_implement_missing_members_skips_members_with_invalid_type() {
        CodeActionRequest::at(&format!(
            "{SIGNATURE_INTERFACE}export component TestCase {{\n    implement 🔺Signature <=> self;\n\n    in property <Nonexistent> text;\n}}\n"
        ))
        .experimental().load()
        .assert(
            "Add the missing declarations for 'Signature'",
            Some(lsp_types::CodeActionKind::QUICKFIX),
            &format!(
                r#"{SIGNATURE_INTERFACE}export component TestCase {{
    implement Signature <=> self;
    public function add(a: int, b: int) -> int {{ }}
    in property <bool> checkable;
    callback clicked();

    in property <Nonexistent> text;
}}
"#
            ),
        ).skip_compile("the invalid member stays");
    }
}
