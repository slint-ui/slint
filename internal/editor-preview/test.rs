// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
};

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_live_preview::protocol::SourceFileVersion;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

async fn parse_source(
    include_paths: Vec<PathBuf>,
    library_paths: HashMap<String, PathBuf>,
    url: lsp_types::Url,
    source_code: String,
    style: String,
    enable_experimental: bool,
    file_loader_fallback: impl Fn(
        &Path,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<Output = Option<std::io::Result<(SourceFileVersion, String)>>>,
        >,
    > + 'static,
) -> (BuildDiagnostics, crate::DocumentCache) {
    let config = {
        let mut tmp = crate::document_cache::CompilerConfiguration::default();
        if !style.is_empty() {
            tmp.style = Some(style);
        }
        tmp.include_paths = include_paths;
        tmp.library_paths = library_paths;
        tmp.enable_experimental |= enable_experimental;
        tmp.open_import_callback = Some(Rc::new(move |path| {
            let path = PathBuf::from(&path);
            file_loader_fallback(&path)
        }));
        // The preview's resource URL mapper is installed by the wasm application at
        // runtime, so it was never set when this fixture ran.
        tmp.resource_url_mapper = None;
        tmp
    };

    let mut document_cache = crate::DocumentCache::new(config);
    let mut diag = i_slint_compiler::diagnostics::BuildDiagnostics::default();

    document_cache.load_url(&url, None, source_code, &mut diag).await.unwrap();

    (diag, document_cache)
}

pub fn test_file_prefix() -> PathBuf {
    #[cfg(windows)]
    return std::path::PathBuf::from("Z:\\");
    #[cfg(not(windows))]
    return std::path::PathBuf::from("/");
}
pub fn main_test_file_name() -> PathBuf {
    test_file_name("test_data.slint")
}

pub fn test_file_name(name: &str) -> PathBuf {
    test_file_prefix().join(name)
}

#[track_caller]
pub fn compile_test_with_sources(
    style: &str,
    code: HashMap<lsp_types::Url, String>,
    allow_warnings: bool,
) -> crate::DocumentCache {
    i_slint_backend_testing::init_no_event_loop();
    recompile_test_with_sources(style, code, allow_warnings, false)
}

/// Like [`compile_test_with_sources`], but with experimental compiler features enabled.
#[track_caller]
pub fn compile_test_with_sources_experimental(
    style: &str,
    code: HashMap<lsp_types::Url, String>,
    allow_warnings: bool,
) -> crate::DocumentCache {
    i_slint_backend_testing::init_no_event_loop();
    recompile_test_with_sources(style, code, allow_warnings, true)
}

pub fn recompile_test_with_sources(
    style: &str,
    code: HashMap<lsp_types::Url, String>,
    allow_warnings: bool,
    enable_experimental: bool,
) -> crate::DocumentCache {
    let code = Rc::new(code);

    let url = lsp_types::Url::from_file_path(main_test_file_name()).unwrap();
    let source_code = code.get(&url).unwrap().clone();
    let (diagnostics, type_loader) = spin_on::spin_on(parse_source(
        Vec::new(),
        std::collections::HashMap::new(),
        url,
        source_code.to_string(),
        style.to_string(),
        enable_experimental,
        move |path| {
            let code = code.clone();
            let url = lsp_types::Url::from_file_path(path);

            Box::pin(async move {
                if let Ok(url) = url {
                    let Some(source) = code.get(&url) else {
                        return Some(Result::Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "path not found",
                        )));
                    };
                    Some(Ok((Some(23), source.clone())))
                } else {
                    Some(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "URL conversion failed",
                    )))
                }
            })
        },
    ));

    tracing::debug!("Test source diagnostics:");
    for d in diagnostics.iter() {
        tracing::debug!("    {:?}: {d}", d.level());
    }
    assert!(!diagnostics.has_errors());
    if !allow_warnings {
        assert!(diagnostics.is_empty());
    }

    type_loader
}

/// Create an empty `DocumentCache`
pub fn empty_document_cache() -> crate::DocumentCache {
    let config = crate::document_cache::CompilerConfiguration {
        style: Some("fluent".to_string()),
        ..Default::default()
    };
    crate::DocumentCache::new(config)
}

/// Create an empty `DocumentCache` with experimental features enabled.
pub fn empty_document_cache_with_experimental() -> crate::DocumentCache {
    let config = crate::document_cache::CompilerConfiguration {
        style: Some("fluent".to_string()),
        enable_experimental: true,
        ..Default::default()
    };
    crate::DocumentCache::new(config)
}

/// Create an `EditorSession` around `document_cache` that sends nowhere.
pub fn session_with(document_cache: crate::DocumentCache) -> crate::EditorSession {
    crate::EditorSession {
        document_cache,
        preview_config: Default::default(),
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        to_show: None,
        open_urls: Default::default(),
        to_preview: crate::LspToPreviews::with_one(crate::DummyLspToPreview::default()),
        pending_recompile: Default::default(),
    }
}

/// Create a `DocumentCache` with one document loaded into it.
pub fn loaded_document_cache(
    content: String,
) -> (crate::DocumentCache, lsp_types::Url, HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>) {
    loaded_document_cache_with_file_name(content, "bar.slint")
}

pub fn loaded_document_cache_with_file_name(
    content: String,
    file_name: &str,
) -> (crate::DocumentCache, lsp_types::Url, HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>) {
    load_content_with_document_cache(empty_document_cache(), content, file_name)
}

pub fn loaded_document_cache_with_experimental(
    content: String,
) -> (crate::DocumentCache, lsp_types::Url, HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>) {
    load_content_with_document_cache(empty_document_cache_with_experimental(), content, "bar.slint")
}

fn load_content_with_document_cache(
    mut document_cache: crate::DocumentCache,
    content: String,
    file_name: &str,
) -> (crate::DocumentCache, lsp_types::Url, HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>) {
    // Pre-load std-widgets.slint:
    spin_on::spin_on(document_cache.preload_builtins());

    let dummy_absolute_path = if cfg!(target_family = "windows") {
        format!("c://foo/{file_name}")
    } else {
        format!("/foo/{file_name}")
    };
    let url = lsp_types::Url::from_file_path(dummy_absolute_path).unwrap();
    let mut session = session_with(document_cache);
    let (extra_files, diag) =
        spin_on::spin_on(session.load_document_impl(content, url.clone(), Some(42)));

    let diag = crate::editor_session::convert_diagnostics(
        &extra_files,
        diag,
        session.document_cache.format,
    );
    (session.document_cache, url, diag)
}

/// Create a `DocumentCache` with one comparatively complex test document loaded into it.
pub fn complex_document_cache()
-> (crate::DocumentCache, lsp_types::Url, HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>) {
    loaded_document_cache(
            r#"import { LineEdit, Button, Slider, HorizontalBox, VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    property <duration> total-time: slider.value * 1s;
    property <duration> elapsed-time;

    callback tick(duration);
    tick(passed-time) => {
        elapsed-time += passed-time;
        elapsed-time = min(elapsed-time, total-time);
    }

    VerticalBox {
        HorizontalBox {
            padding-left: 0;
            Text { text: "Elapsed Time:"; }
            Rectangle {
                min-width: 200px;
                max-height: 30px;
                background: gray;
                Rectangle {
                    height: 100%;
                    width: parent.width * (elapsed-time/total-time);
                    background: lightblue;
                }
            }
        }
        Text{
            text: (total-time / 1s) + "s";
        }
        HorizontalBox {
            padding-left: 0;
            Text {
                text: "Duration:";
                vertical-alignment: center;
            }
            slider := Slider {
                maximum: 30s / 1s;
                value: 10s / 1s;
                changed(new-duration) => {
                    root.total-time = new-duration * 1s;
                    root.elapsed-time = min(root.elapsed-time, root.total-time);
                }
            }
        }
        Button {
            text: "Reset";
            clicked => {
                elapsed-time = 0
            }
        }
    }
}
            "#.to_string())
}

pub fn load(
    session: &mut crate::EditorSession,
    path: &Path,
    content: &str,
) -> (lsp_types::Url, HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>) {
    let url = lsp_types::Url::from_file_path(path).unwrap();

    let (main_file, diag) =
        spin_on::spin_on(session.load_document_impl(content.into(), url.clone(), Some(1)));

    (
        url,
        crate::editor_session::convert_diagnostics(&main_file, diag, session.document_cache.format),
    )
}
