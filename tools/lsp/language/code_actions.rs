// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod element;
mod import;
mod qualify;
#[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
mod show_preview;
#[cfg(test)]
pub mod test;
mod translate;

use super::completion;
use crate::editor_preview::{self, DocumentCache};
use crate::util;

use i_slint_compiler::object_tree::QualifiedTypeName;
use i_slint_compiler::parser::{SyntaxKind, SyntaxNode, SyntaxToken, syntax_nodes};
use lsp_types::{ClientCapabilities, CodeActionKind, CodeActionOrCommand, TextEdit, Url};

struct Context<'a> {
    document_cache: &'a DocumentCache,
    token: SyntaxToken,
    node: SyntaxNode,
    uri: Url,
    component: Option<syntax_nodes::Component>,
    snippets: bool,
}

impl<'a> Context<'a> {
    fn new(
        document_cache: &'a DocumentCache,
        token: SyntaxToken,
        client_capabilities: &ClientCapabilities,
    ) -> Option<Self> {
        let node = token.parent();
        let uri = token.source_file.path().to_url()?;
        let component = syntax_nodes::Component::new(node.clone())
            .or_else(|| {
                syntax_nodes::DeclaredIdentifier::new(node.clone())
                    .and_then(|n| n.parent())
                    .and_then(syntax_nodes::Component::new)
            })
            .or_else(|| {
                syntax_nodes::QualifiedName::new(node.clone())
                    .and_then(|n| n.parent())
                    .and_then(syntax_nodes::Element::new)
                    .and_then(|n| n.parent())
                    .and_then(syntax_nodes::Component::new)
            })
            .or_else(|| syntax_nodes::ExportsList::new(node.clone()).and_then(|n| n.Component()))
            .filter(|c| c.child_text(SyntaxKind::Identifier).is_none_or(|t| t != "global"));
        let snippets = has_experimental_client_capability(client_capabilities, "snippetTextEdit");
        Some(Self { document_cache, token, node, uri, component, snippets })
    }

    fn push_edit(
        &self,
        result: &mut Vec<CodeActionOrCommand>,
        title: impl Into<String>,
        kind: Option<CodeActionKind>,
        edits: Vec<TextEdit>,
    ) {
        result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
            title: title.into(),
            kind,
            edit: editor_preview::editing::create_workspace_edit_from_path(
                self.document_cache,
                self.token.source_file.path(),
                edits,
            ),
            ..Default::default()
        }));
    }
}

fn has_experimental_client_capability(capabilities: &ClientCapabilities, name: &str) -> bool {
    capabilities
        .experimental
        .as_ref()
        .and_then(|o| o.get(name).and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

pub fn get_code_actions(
    document_cache: &DocumentCache,
    token: SyntaxToken,
    client_capabilities: &ClientCapabilities,
) -> Option<Vec<CodeActionOrCommand>> {
    let ctx = Context::new(document_cache, token, client_capabilities)?;
    let token = &ctx.token;
    let node = &ctx.node;
    let mut result = Vec::new();

    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    show_preview::add(&ctx, &mut result);

    translate::add(&ctx, &mut result);
    import::add(&ctx, &mut result);
    element::add(&ctx, &mut result);
    qualify::add(&ctx, &mut result);

    if token.kind() == SyntaxKind::Identifier
        && let Some(implement_statement) = match node.kind() {
            SyntaxKind::ImplementStatement => syntax_nodes::ImplementStatement::new(node.clone()),
            SyntaxKind::QualifiedName | SyntaxKind::DeclaredIdentifier
                if node.parent().map(|n| n.kind()) == Some(SyntaxKind::ImplementStatement) =>
            {
                node.parent().and_then(syntax_nodes::ImplementStatement::new)
            }
            _ => None,
        }
    {
        let r = util::text_range_to_lsp_range(
            &token.source_file,
            implement_statement.text_range(),
            document_cache.format,
        );
        if let Some(element) = document_cache.element_at_position(&ctx.uri, &r.start) {
            let missing =
                i_slint_compiler::object_tree::missing_interface_members(&element.element);
            if let Some(missing_entry) = missing
                .into_iter()
                .find(|entry| entry.statement.text_range() == implement_statement.text_range())
            {
                let indent = util::find_indent(&implement_statement).unwrap_or_default();
                let insertion: String = missing_entry
                    .declarations
                    .iter()
                    .map(|decl| format!("\n{indent}{decl}"))
                    .collect();
                let insert_pos = util::text_size_to_lsp_position(
                    &token.source_file,
                    implement_statement.text_range().end(),
                    document_cache.format,
                );
                let interface_name =
                    QualifiedTypeName::from_node(implement_statement.QualifiedName());
                ctx.push_edit(
                    &mut result,
                    format!("Add the missing declarations for '{interface_name}'"),
                    Some(CodeActionKind::QUICKFIX),
                    vec![TextEdit::new(lsp_types::Range::new(insert_pos, insert_pos), insertion)],
                );
            }
        }
    }

    completion::match_element::add_code_actions(document_cache, token, &mut result);

    (!result.is_empty()).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::test::CodeActionRequest;

    #[test]
    fn test_code_action_no_show_preview_for_struct_or_global() {
        CodeActionRequest::at("export struct 🔺NoPreviewForStruct { x: int }")
            .load()
            .assert_empty();
        CodeActionRequest::at("export global 🔺NoPreviewForGlobal {}").load().assert_empty();
    }

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
