// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::JsDiagnostic;
use i_slint_compiler::CompilerConfiguration;
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::generator::typescript::is_declaration_file_name;
use i_slint_compiler::project_file::{self, Overrides, ProjectFileData};
use std::collections::HashMap;
use std::path::PathBuf;

#[napi(object)]
pub struct TypeScriptDeclarations {
    /// The content of the `.d.ts` file, absent if the `.slint` file has errors.
    pub declarations: Option<String>,
    pub diagnostics: Vec<JsDiagnostic>,
}

/// Generates the TypeScript declarations of the `.slint` file, or `slint-project.json`, at `path`,
/// as `slint-compiler -f typescript` does.
/// Writes them to `output`, `-` to only return them, or else next to the `.slint` file as `<file>.d.ts`.
#[napi]
pub fn generate_typescript_declarations(
    path: String,
    output: Option<String>,
    include_paths: Vec<String>,
    library_paths: HashMap<String, String>,
) -> napi::Result<TypeScriptDeclarations> {
    let (path, project_file) =
        project_file::resolve_input(path.as_ref()).map_err(napi::Error::from_reason)?;
    let output = match output.as_deref() {
        Some("-") => None,
        Some(output) => Some(PathBuf::from(output)),
        None => Some(PathBuf::from(format!("{}.d.ts", path.display()))),
    };
    if let Some(name) = output.as_ref().and_then(|output| output.file_name()?.to_str())
        && !is_declaration_file_name(name)
    {
        return Err(napi::Error::from_reason(format!(
            "The TypeScript output is a declaration file: name it '{name}.d.ts'"
        )));
    }

    let mut config = CompilerConfiguration::new(OutputFormat::TypeScript);
    (config.include_paths, config.library_paths) = crate::paths_from_env();
    let overrides = Overrides {
        project: ProjectFileData {
            include_paths: (!include_paths.is_empty())
                .then(|| include_paths.into_iter().map(PathBuf::from).collect()),
            library_paths: (!library_paths.is_empty()).then(|| {
                library_paths.into_iter().map(|(name, path)| (name, PathBuf::from(path))).collect()
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    overrides.apply(project_file.as_ref(), &mut config);

    let mut diag = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse_file(&path, &mut diag);
    let mut declarations = None;
    if let Some(syntax_node) = syntax_node.filter(|_| !diag.has_errors()) {
        let (doc, compile_diag, loader) =
            spin_on::spin_on(i_slint_compiler::compile_syntax_node(syntax_node, diag, config));
        diag = compile_diag;
        if !diag.has_errors() {
            let mut content = Vec::new();
            i_slint_compiler::generator::generate(
                OutputFormat::TypeScript,
                &mut content,
                None,
                &doc,
                &loader.compiler_config,
            )?;
            if let Some(output) = &output {
                i_slint_compiler::fileaccess::write_file_if_changed(output, &content)?;
            }
            declarations = Some(
                String::from_utf8(content).map_err(|e| napi::Error::from_reason(e.to_string()))?,
            );
        }
    }

    Ok(TypeScriptDeclarations {
        declarations,
        diagnostics: diag.into_iter().map(JsDiagnostic::from).collect(),
    })
}
