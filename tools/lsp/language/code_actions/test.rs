// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use lsp_types::{CodeActionOrCommand, Url};

use crate::editor_preview;
use crate::language::test::{empty_document_cache, empty_document_cache_with_experimental};

const CURSOR_EMOJI: char = '🔺';

pub struct CodeActionRequest {
    source: String,
    cursor: usize,
    snippets: bool,
    experimental: bool,
    extra_files: Vec<(&'static str, String)>,
}

impl CodeActionRequest {
    /// The code actions at the `🔺` in `source_with_cursor`.
    pub fn at(source_with_cursor: &str) -> Self {
        let cursor = source_with_cursor.find(CURSOR_EMOJI).expect("no cursor in the source");
        Self::at_offset(&source_with_cursor.replacen(CURSOR_EMOJI, "", 1), cursor)
    }

    pub fn at_offset(source: &str, cursor: usize) -> Self {
        Self {
            source: source.into(),
            cursor,
            snippets: false,
            experimental: false,
            extra_files: Vec::new(),
        }
    }

    pub fn snippets(mut self) -> Self {
        self.snippets = true;
        self
    }

    pub fn experimental(mut self) -> Self {
        self.experimental = true;
        self
    }

    pub fn with_file(mut self, name: &'static str, content: &str) -> Self {
        self.extra_files.push((name, content.into()));
        self
    }

    pub fn load(self) -> CodeActions {
        let mut document_cache = if self.experimental {
            empty_document_cache_with_experimental()
        } else {
            empty_document_cache()
        };
        spin_on::spin_on(document_cache.preload_builtins());
        let mut diagnostics = i_slint_compiler::diagnostics::BuildDiagnostics::default();
        for (name, content) in self.extra_files {
            let url = editor_preview::test::test_file_name(name).to_url().unwrap();
            spin_on::spin_on(document_cache.load_url(&url, Some(1), content, &mut diagnostics))
                .unwrap();
        }
        let url = editor_preview::test::main_test_file_name().to_url().unwrap();
        spin_on::spin_on(document_cache.load_url(&url, Some(42), self.source, &mut diagnostics))
            .unwrap();

        let mut capabilities = lsp_types::ClientCapabilities::default();
        if self.snippets {
            capabilities.experimental = Some(serde_json::json!({"snippetTextEdit": true}));
        }
        let document = document_cache.get_document(&url).unwrap();
        let token = crate::language::token_at_offset(
            document.node.as_ref().unwrap(),
            (self.cursor as u32).into(),
        )
        .unwrap();
        let actions =
            super::get_code_actions(&mut document_cache, token, &capabilities).unwrap_or_default();
        CodeActions { document_cache, url, cursor: self.cursor, actions }
    }
}

pub struct CodeActions {
    document_cache: editor_preview::DocumentCache,
    url: Url,
    cursor: usize,
    actions: Vec<CodeActionOrCommand>,
}

impl CodeActions {
    /// Clients show the actions in this order.
    pub fn titles(&self) -> Vec<&str> {
        self.actions
            .iter()
            .map(|action| match action {
                CodeActionOrCommand::CodeAction(action) => action.title.as_str(),
                CodeActionOrCommand::Command(command) => command.title.as_str(),
            })
            .collect()
    }

    #[track_caller]
    pub fn assert_empty(&self) {
        assert!(
            self.actions.is_empty(),
            "expected no actions at offset {}, got {:?}",
            self.cursor,
            self.titles()
        );
    }

    #[track_caller]
    pub fn assert(
        &self,
        title: &str,
        kind: Option<lsp_types::CodeActionKind>,
        expected: &str,
    ) -> EditedSource<'_> {
        let mut matching = self.actions.iter().filter_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) if action.title == title => Some(action),
            _ => None,
        });
        let (Some(action), None) = (matching.next(), matching.next()) else {
            panic!("expected one action {title:?}, got {:?}", self.titles());
        };
        assert_eq!(action.kind, kind, "kind of {title:?}");
        let edited = editor_preview::editing::text_edit::apply_workspace_edit(
            &self.document_cache,
            action.edit.as_ref().expect("action without an edit"),
        )
        .unwrap();
        let Ok([edited]) = <[_; 1]>::try_from(edited) else {
            panic!("{title:?} edits more than one document");
        };
        assert_eq!(edited.url, self.url);
        assert_eq!(edited.contents, expected, "result of {title:?}");
        EditedSource { document_cache: &self.document_cache, edited }
    }

    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    pub fn command(&self, title: &str) -> &lsp_types::Command {
        self.actions
            .iter()
            .find_map(|action| match action {
                CodeActionOrCommand::Command(command) if command.title == title => Some(command),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no command {title:?}, got {:?}", self.titles()))
    }
}

#[must_use = "check that the result compiles, or skip it with a reason"]
pub struct EditedSource<'a> {
    document_cache: &'a editor_preview::DocumentCache,
    edited: editor_preview::editing::text_edit::EditedText,
}

impl EditedSource<'_> {
    #[track_caller]
    pub fn compiles(self) {
        self.compiles_with(&[]);
    }

    #[track_caller]
    pub fn compiles_with(mut self, fill_ins: &[(&str, &str)]) {
        let mut unused: Vec<&str> = fill_ins.iter().map(|(default, _)| *default).collect();
        let mut filled = String::new();
        let mut rest = self.edited.contents.as_str();
        while let Some(start) = rest.find('$') {
            filled.push_str(&rest[..start]);
            rest = &rest[start + 1..];
            if let Some(placeholder) = rest.strip_prefix('{') {
                let end = placeholder.find('}').expect("unterminated snippet placeholder");
                let (_, default) = placeholder[..end]
                    .split_once(':')
                    .expect("snippet placeholder without default");
                unused.retain(|unused| *unused != default);
                filled.push_str(
                    fill_ins
                        .iter()
                        .find(|(placeholder_default, _)| *placeholder_default == default)
                        .map_or(default, |(_, fill_in)| fill_in),
                );
                rest = &placeholder[end + 1..];
            } else {
                rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
            }
        }
        filled.push_str(rest);
        assert!(unused.is_empty(), "no snippet placeholders for the fill-ins {unused:?}");
        self.edited.contents = filled;
        editor_preview::test::recompile_test_with_edits(
            self.document_cache,
            std::slice::from_ref(&self.edited),
            true,
        );
    }

    pub fn skip_compile(self, _reason: &str) {}
}
