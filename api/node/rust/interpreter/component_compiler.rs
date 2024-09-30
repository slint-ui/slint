// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::JsComponentDefinition;
use super::JsDiagnostic;
use itertools::Itertools;
use slint_interpreter::Compiler;
use std::collections::HashMap;
use std::path::PathBuf;

/// ComponentCompiler is the entry point to the Slint interpreter that can be used
/// to load .slint files or compile them on-the-fly from a string.
#[napi(js_name = "ComponentCompiler")]
pub struct JsComponentCompiler {
    internal: Compiler,
    diagnostics: Vec<slint_interpreter::Diagnostic>,
}

#[napi]
impl JsComponentCompiler {
    /// Returns a new ComponentCompiler.
    #[napi(constructor)]
    pub fn new() -> Self {
        let mut compiler = Compiler::default();
        let include_paths = match std::env::var_os("SLINT_INCLUDE_PATH") {
            Some(paths) => {
                std::env::split_paths(&paths).filter(|path| !path.as_os_str().is_empty()).collect()
            }
            None => vec![],
        };
        let library_paths = match std::env::var_os("SLINT_LIBRARY_PATH") {
            Some(paths) => std::env::split_paths(&paths)
                .filter_map(|entry| {
                    entry
                        .to_str()
                        .unwrap_or_default()
                        .split('=')
                        .collect_tuple()
                        .map(|(k, v)| (k.into(), v.into()))
                })
                .collect(),
            None => HashMap::new(),
        };

        compiler.set_include_paths(include_paths);
        compiler.set_library_paths(library_paths);
        Self { internal: compiler, diagnostics: vec![] }
    }

    #[napi(setter)]
    pub fn set_include_paths(&mut self, include_paths: Vec<String>) {
        self.internal.set_include_paths(include_paths.iter().map(|p| PathBuf::from(p)).collect());
    }

    #[napi(getter)]
    pub fn include_paths(&self) -> Vec<String> {
        self.internal
            .include_paths()
            .iter()
            .map(|p| p.to_str().unwrap_or_default().to_string())
            .collect()
    }

    #[napi(setter)]
    pub fn set_library_paths(&mut self, paths: HashMap<String, String>) {
        let mut library_paths = HashMap::new();
        for (key, path) in paths {
            library_paths.insert(key, PathBuf::from(path));
        }

        self.internal.set_library_paths(library_paths);
    }

    #[napi(getter)]
    pub fn library_paths(&self) -> HashMap<String, String> {
        let mut library_paths = HashMap::new();

        for (key, path) in self.internal.library_paths() {
            library_paths.insert(key.clone(), path.to_str().unwrap_or_default().to_string());
        }

        library_paths
    }

    #[napi(setter)]
    pub fn set_style(&mut self, style: String) {
        self.internal.set_style(style);
    }

    #[napi(getter)]
    pub fn style(&self) -> Option<String> {
        self.internal.style().cloned()
    }

    // todo: set_file_loader

    #[napi(getter)]
    pub fn diagnostics(&self) -> Vec<JsDiagnostic> {
        self.diagnostics.iter().map(|d| JsDiagnostic::from(d.clone())).collect()
    }

    /// Compile a .slint file into a ComponentDefinition
    ///
    /// Returns the compiled `ComponentDefinition` if there were no errors.
    #[napi]
    pub fn build_from_path(&mut self, path: String) -> HashMap<String, JsComponentDefinition> {
        let r = spin_on::spin_on(self.internal.build_from_path(PathBuf::from(path)));
        self.diagnostics = r.diagnostics().collect();
        r.components().map(|c| (c.name().to_owned(), c.into())).collect()
    }

    /// Compile some .slint code into a ComponentDefinition
    #[napi]
    pub fn build_from_source(
        &mut self,
        source_code: String,
        path: String,
    ) -> HashMap<String, JsComponentDefinition> {
        let r = spin_on::spin_on(self.internal.build_from_source(source_code, PathBuf::from(path)));
        self.diagnostics = r.diagnostics().collect();
        r.components().map(|c| (c.name().to_owned(), c.into())).collect()
    }
}
