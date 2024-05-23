// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
};

use i_slint_compiler::{diagnostics::BuildDiagnostics, typeloader::TypeLoader};

async fn parse_source(
    include_paths: Vec<PathBuf>,
    library_paths: HashMap<String, PathBuf>,
    path: PathBuf,
    source_code: String,
    style: String,
    file_loader_fallback: impl Fn(
            &Path,
        ) -> core::pin::Pin<
            Box<dyn core::future::Future<Output = Option<std::io::Result<String>>>>,
        > + 'static,
) -> (BuildDiagnostics, TypeLoader) {
    let config = {
        let mut tmp = i_slint_compiler::CompilerConfiguration::new(
            i_slint_compiler::generator::OutputFormat::Llr,
        );
        if !style.is_empty() {
            tmp.style = Some(style);
        }
        tmp.include_paths = include_paths;
        tmp.library_paths = library_paths;
        tmp.open_import_fallback =
            Some(Rc::new(move |path| file_loader_fallback(Path::new(path.as_str()))));
        #[cfg(target_arch = "wasm32")]
        {
            tmp.resource_url_mapper = resource_url_mapper();
        }
        tmp
    };
    let mut diag = i_slint_compiler::diagnostics::BuildDiagnostics::default();

    let global_type_registry = i_slint_compiler::typeregister::TypeRegister::builtin();

    let mut type_loader = TypeLoader::new(global_type_registry, config, &mut diag);

    type_loader.load_file(&path, None, &path, source_code, false, &mut diag).await;

    (diag, type_loader)
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
pub fn compile_test_with_sources(style: &str, code: HashMap<PathBuf, String>) -> TypeLoader {
    i_slint_backend_testing::init_no_event_loop();
    recompile_test_with_sources(style, code)
}

#[track_caller]
pub fn recompile_test_with_sources(style: &str, code: HashMap<PathBuf, String>) -> TypeLoader {
    let code = Rc::new(code);

    let path = main_test_file_name();
    let source_code = code.get(&path).unwrap().clone();
    let (diagnostics, type_loader) = spin_on::spin_on(parse_source(
        vec![],
        std::collections::HashMap::new(),
        path,
        source_code.to_string(),
        style.to_string(),
        move |path| {
            let code = code.clone();
            let path = path.to_owned();

            Box::pin(async move {
                let Some(source) = code.get(&path) else {
                    return Some(Result::Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "path not found",
                    )));
                };
                Some(Ok(source.clone()))
            })
        },
    ));

    i_slint_core::debug_log!("Test source diagnostics:");
    for d in diagnostics.iter() {
        i_slint_core::debug_log!("    {d}");
    }
    assert!(diagnostics.is_empty());

    type_loader
}
