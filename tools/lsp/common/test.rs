// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
};

use i_slint_compiler::diagnostics::BuildDiagnostics;

use crate::common;

async fn parse_source(
    include_paths: Vec<PathBuf>,
    library_paths: HashMap<String, PathBuf>,
    url: lsp_types::Url,
    source_code: String,
    style: String,
    file_loader_fallback: impl Fn(
            &Path,
        ) -> core::pin::Pin<
            Box<
                dyn core::future::Future<
                    Output = Option<std::io::Result<(common::SourceFileVersion, String)>>,
                >,
            >,
        > + 'static,
) -> (BuildDiagnostics, common::DocumentCache) {
    let config = {
        let mut tmp = common::document_cache::CompilerConfiguration::default();
        if !style.is_empty() {
            tmp.style = Some(style);
        }
        tmp.include_paths = include_paths;
        tmp.library_paths = library_paths;
        tmp.open_import_fallback = Some(Rc::new(move |path| {
            let path = PathBuf::from(&path);
            file_loader_fallback(&path)
        }));
        #[cfg(target_arch = "wasm32")]
        {
            tmp.resource_url_mapper = crate::preview::connector::resource_url_mapper();
        }
        tmp
    };

    let mut document_cache = common::DocumentCache::new(config);
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
) -> common::DocumentCache {
    i_slint_backend_testing::init_no_event_loop();
    recompile_test_with_sources(style, code, allow_warnings)
}

#[track_caller]
pub fn recompile_test_with_sources(
    style: &str,
    code: HashMap<lsp_types::Url, String>,
    allow_warnings: bool,
) -> common::DocumentCache {
    let code = Rc::new(code);

    let url = lsp_types::Url::from_file_path(main_test_file_name()).unwrap();
    let source_code = code.get(&url).unwrap().clone();
    let (diagnostics, type_loader) = spin_on::spin_on(parse_source(
        vec![],
        std::collections::HashMap::new(),
        url,
        source_code.to_string(),
        style.to_string(),
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

    eprintln!("Test source diagnostics:");
    for d in diagnostics.iter() {
        eprintln!("    {:?}: {d}", d.level());
    }
    assert!(!diagnostics.has_errors());
    if !allow_warnings {
        assert!(diagnostics.is_empty());
    }

    type_loader
}
