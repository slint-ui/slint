// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code to help with writing tests for the language server

use lsp_types::{CodeActionOrCommand, Url};

use i_slint_live_preview::file_watcher::FileChangeKind;

use std::collections::HashMap;

use crate::editor_preview;
use crate::editor_preview::LspToPreviews;

// Fixtures for the shared document model live with it.
#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub use i_slint_editor_preview::test::preview_capture;
pub use i_slint_editor_preview::test::{
    complex_document_cache, empty_document_cache, empty_document_cache_with_experimental, load,
    loaded_document_cache, loaded_document_cache_with_experimental,
    loaded_document_cache_with_file_name,
};

use super::Context;

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
        let token =
            super::token_at_offset(document.node.as_ref().unwrap(), (self.cursor as u32).into())
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

pub fn mock_context() -> Context {
    mock_context_with_document_cache(empty_document_cache())
}

pub fn mock_context_with_document_cache(document_cache: editor_preview::DocumentCache) -> Context {
    crate::language::Context {
        session: editor_preview::EditorSession::new(
            document_cache,
            LspToPreviews::with_one(editor_preview::DummyLspToPreview::default()),
        ),
        server_notifier: crate::ServerNotifier::dummy(),
        init_param: Default::default(),
        host_language_rename_dont_ask_again: Default::default(),
    }
}

#[test]
fn accurate_diagnostics_in_dependencies() {
    // Test for issue 5797
    let mut ctx = mock_context();

    let (bar_url, diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/bar.slint"),
        r#" export component Bar { property <int> hi; } "#,
    );
    assert_eq!(diag, HashMap::from_iter([(bar_url.clone(), Vec::new())]));

    let (reexport_url, diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/reexport.slint"),
        r#"import { Bar } from "bar.slint"; export component Foo inherits Bar { in property <string> reexport; }"#,
    );
    assert_eq!(diag, HashMap::from_iter([(reexport_url.clone(), Vec::new())]));

    let (foo_url, diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/foo.slint"),
        r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hello: 45; } }"#,
    );

    assert!(diag[&foo_url][0].message.contains("hello"));
    assert_eq!(diag.len(), 1);

    ctx.session.open_urls.insert(foo_url.clone());
    ctx.session.open_urls.insert(bar_url.clone());

    let (bar_url, diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/bar.slint"),
        r#" export component Bar { in property <int> hello; } "#,
    );
    assert_eq!(diag.len(), 3);
    assert_eq!(
        diag,
        HashMap::from_iter([
            (reexport_url.clone(), Vec::new()),
            (bar_url.clone(), Vec::new()),
            (foo_url.clone(), Vec::new())
        ])
    );

    let sym = crate::language::get_document_symbols(
        &mut ctx.session.document_cache,
        &lsp_types::TextDocumentIdentifier { uri: foo_url.clone() },
    )
    .expect("foo.slint should still be loaded");
    assert!(matches!(sym, lsp_types::DocumentSymbolResponse::Nested(result) if !result.is_empty()));

    let (foo_url, diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/foo.slint"),
        r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hi: 45; } }"#,
    );
    assert!(diag[&foo_url][0].message.contains("hi"));

    let (foo_url, diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/foo.slint"),
        r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hello: 12; } }"#,
    );
    assert_eq!(diag[&foo_url], Vec::new());
}

#[test]
fn accurate_diagnostics_in_dependencies_with_parse_errors() {
    // Test for issue 8064
    let mut ctx = mock_context();

    let (bar_url, diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/bar.slint"),
        r#" export component Bar { in property <int> hello; } "#,
    );
    assert_eq!(diag, HashMap::from_iter([(bar_url.clone(), Vec::new())]));

    ctx.session.open_urls.insert(bar_url.clone());

    let (reexport_url, diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/reexport.slint"),
        r#"import { Bar } from "bar.slint"; export component Foo inherits Bar { in property <string> reexport; if true error }"#,
    );
    assert!(diag[&reexport_url].iter().any(|d| d.message.contains("Syntax error:")));
    assert_eq!(diag.len(), 1);

    ctx.session.open_urls.insert(reexport_url.clone());

    let (foo_url, diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/foo.slint"),
        r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hello: 45; world: 12; } }"#,
    );
    assert!(diag[&foo_url][0].message.contains("world"));
    assert_eq!(diag[&foo_url].len(), 1);
    // Don't clear further error (so the client still has the parse error in reexport_url)
    assert_eq!(diag.len(), 1);

    ctx.session.open_urls.insert(foo_url.clone());

    let (bar_url, diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/bar.slint"),
        r#" export component Bar { private property <int> hello; in property <int> world; } "#,
    );

    // bar still don't have error
    assert_eq!(diag[&bar_url], Vec::new());
    // But reexport_url still have the same syntax error as before
    assert!(diag[&reexport_url].iter().any(|d| d.message.contains("Syntax error:")));
}

/// Test for issue #10521: Preview file should be recompiled when dependency changes,
/// even if the preview file is not open in the editor.
#[test]
#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
fn preview_file_recompiled_when_dependency_changes() {
    let mut ctx = mock_context();

    let (dep_url, _diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/bar.slint"),
        r#" export component Bar { property <int> hi; } "#,
    );

    let (main_url, _diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/main.slint"),
        r#"import { Dep } from "bar.slint"; export component Main { Dep { } }"#,
    );

    let (secondary_url, _diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/secondary.slint"),
        r#"import { Dep } from "bar.slint"; export component Secondary { Dep { } }"#,
    );

    // Update context with:
    // - main.slint set as the preview file (to_show)
    // - main.slint NOT in open_urls (simulating it was closed in the editor)
    ctx.session.primary_preview_mut().to_show =
        Some(i_slint_live_preview::protocol::PreviewComponent {
            url: main_url.clone(),
            component: None,
        });
    ctx.session.previews.push(editor_preview::PreviewConnection {
        to_preview: editor_preview::LspToPreviews::with_one(
            editor_preview::DummyLspToPreview::default(),
        ),
        to_show: Some(i_slint_live_preview::protocol::PreviewComponent {
            url: secondary_url.clone(),
            component: None,
        }),
    });

    spin_on::spin_on(ctx.session.trigger_file_watcher(dep_url.clone(), FileChangeKind::Changed))
        .unwrap();

    // The preview file (main.slint) should be scheduled for recompilation
    // even though it's not in open_urls
    assert!(
        ctx.session.pending_recompile.contains(&main_url),
        "Preview file should be in pending_recompile when its dependency changes"
    );
    assert!(
        ctx.session.pending_recompile.contains(&secondary_url),
        "Secondary preview file should be in pending_recompile when its dependency changes"
    );
}

#[test]
#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
fn request_state_re_sends_only_targeted_files_when_present() {
    let (capture, messages) = preview_capture();
    let mut ctx = mock_context();

    let (url, _diag) = load(
        &mut ctx.session,
        &std::env::current_dir().unwrap().join("xxx/main.slint"),
        r#"export component Main { }"#,
    );

    ctx.session.primary_preview_mut().to_preview = capture;
    messages.borrow_mut().clear();

    crate::language::send_requested_state_to_preview(&ctx, &[url], &[]);

    let messages = messages.borrow();
    assert_eq!(messages.len(), 1);
    assert!(matches!(
        messages[0],
        i_slint_live_preview::protocol::LspToPreviewMessage::SetContents { .. }
    ));
}

/// Test for issue #11304
/// When a file is renamed in the editor first, and then is renamed on disk accordingly (i.e.
/// "appears" as a new file).
mod missing_imports {
    use std::path::PathBuf;

    use super::*;

    fn load_document_with_missing_import() -> (Context, PathBuf, Url) {
        let mut ctx = mock_context();

        let dir = std::env::current_dir().unwrap().join("xxx");

        // Load main.slint that imports dep.slint, which does not yet exist.
        let (main_url, diag) = load(
            &mut ctx.session,
            &dir.join("main.slint"),
            r#"import { Dep } from "dep.slint"; export component Main { Dep { } }"#,
        );
        assert!(
            !diag[&main_url].is_empty(),
            "Expected diagnostics for missing import, got: {diag:?}"
        );
        (ctx, dir, main_url)
    }

    #[test]
    fn created_in_editor() {
        let (mut ctx, dir, main_url) = load_document_with_missing_import();

        // Now "create" dep.slint by opening it (simulating a DidOpenTextDocument / file rename).
        let (dep_url, diag) =
            load(&mut ctx.session, &dir.join("dep.slint"), r#"export component Dep { }"#);

        assert!(diag[&dep_url].is_empty(), "dep.slint should have no errors");
        assert!(
            diag[&main_url].is_empty(),
            "main.slint should have no errors after dep.slint is created"
        );
    }

    #[test]
    fn created_outside_editor() {
        let (mut ctx, dir, main_url) = load_document_with_missing_import();

        // Simulate that the file was opened via load_document
        ctx.session.open_urls.insert(main_url.clone());

        let dep_url = Url::from_file_path(dir.join("dep.slint")).unwrap();
        spin_on::spin_on(ctx.session.trigger_file_watcher(dep_url, FileChangeKind::Created))
            .unwrap();

        assert!(
            ctx.session.pending_recompile.contains(&main_url),
            "main.slint should be scheduled for recompilation when dep.slint is created outside the editor"
        );
    }

    #[test]
    fn watch_set_tracks_missing_imports() {
        let (ctx, dir, _) = load_document_with_missing_import();

        let watch_paths = ctx.session.document_cache.all_paths_to_watch();

        assert!(
            watch_paths.contains(&dir.join("main.slint")),
            "main.slint should stay in the watch set"
        );
        assert!(
            watch_paths.contains(&dir.join("dep.slint")),
            "missing imports should stay in the watch set"
        );
    }
}
