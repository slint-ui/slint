// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod element;
mod implement;
mod import;
mod match_element;
mod qualify;
#[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
mod show_preview;
#[cfg(test)]
pub mod test;
mod translate;

use crate::editor_preview::{self, DocumentCache};

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
    let mut result = Vec::new();

    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    show_preview::add(&ctx, &mut result);

    translate::add(&ctx, &mut result);
    import::add(&ctx, &mut result);
    element::add(&ctx, &mut result);
    qualify::add(&ctx, &mut result);
    implement::add(&ctx, &mut result);
    match_element::add(&ctx, &mut result);

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
}
