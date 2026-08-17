// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code to help with writing tests for the language server

use lsp_types::Url;

use i_slint_live_preview::file_watcher::FileChangeKind;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::editor_preview;
use crate::editor_preview::LspToPreviews;

// Fixtures for the shared document model live with it.
pub use i_slint_editor_preview::test::{
    complex_document_cache, empty_document_cache, empty_document_cache_with_experimental, load,
    loaded_document_cache, loaded_document_cache_with_experimental,
    loaded_document_cache_with_file_name,
};

use super::Context;

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
#[derive(Default)]
struct CapturePreview {
    messages: Rc<RefCell<Vec<i_slint_live_preview::protocol::LspToPreviewMessage>>>,
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
impl editor_preview::LspToPreview for CapturePreview {
    fn send(&self, message: &i_slint_live_preview::protocol::LspToPreviewMessage) {
        self.messages.borrow_mut().push(message.clone());
    }

    fn preview_target(&self) -> i_slint_live_preview::protocol::PreviewTarget {
        i_slint_live_preview::protocol::PreviewTarget::Dummy
    }
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub(crate) type CapturedPreviewMessages =
    Rc<RefCell<Vec<i_slint_live_preview::protocol::LspToPreviewMessage>>>;

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub(crate) fn preview_capture() -> (Rc<LspToPreviews>, CapturedPreviewMessages) {
    let capture = CapturePreview::default();
    let messages = capture.messages.clone();
    (LspToPreviews::with_one(capture), messages)
}

pub fn mock_context() -> Context {
    crate::language::Context {
        session: editor_preview::EditorSession {
            document_cache: empty_document_cache(),
            preview_config: Default::default(),
            #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
            to_show: None,
            open_urls: HashSet::new(),
            to_preview: LspToPreviews::with_one(editor_preview::DummyLspToPreview::default()),
            pending_recompile: Default::default(),
        },
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

    // Update context with:
    // - main.slint set as the preview file (to_show)
    // - main.slint NOT in open_urls (simulating it was closed in the editor)
    ctx.session.to_show = Some(i_slint_live_preview::protocol::PreviewComponent {
        url: main_url.clone(),
        component: None,
    });

    spin_on::spin_on(ctx.session.trigger_file_watcher(dep_url.clone(), FileChangeKind::Changed))
        .unwrap();

    // The preview file (main.slint) should be scheduled for recompilation
    // even though it's not in open_urls
    assert!(
        ctx.session.pending_recompile.contains(&main_url),
        "Preview file should be in pending_recompile when its dependency changes"
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

    ctx.session.to_preview = capture;
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
        let (ctx, dir, main_url) = load_document_with_missing_import();

        let dep_url = Url::from_file_path(dir.join("dep.slint")).unwrap();
        let watch_urls = ctx.session.document_cache.all_urls_to_watch();

        assert!(watch_urls.contains(&main_url), "main.slint should stay in the watch set");
        assert!(watch_urls.contains(&dep_url), "missing imports should stay in the watch set");
    }
}
