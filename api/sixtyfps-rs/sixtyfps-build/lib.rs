/*!
    This crate serves as a compagnon crate for the sixtyfps crate.
    It is meant to be able to compile the `.60` files from your `build.rs`script

    The main entry point of this crate is the gernerate() function
*/

#![warn(missing_docs)]

use sixtyfps_compilerlib::*;
use std::env;
use std::io::Write;
use std::path::Path;

/// Error returned by the `compile` function
#[derive(thiserror::Error, Debug)]
pub enum CompileError {
    /// Cannot read environment variable CARGO_MANIFEST_DIR or OUT_DIR. The build script need to be run via cargo.
    #[error("Cannot read environment variable CARGO_MANIFEST_DIR or OUT_DIR. The build script need to be run via cargo.")]
    NotRunViaCargo,
    /// Cannot load the input .60 file
    #[error("Cannot load the .60 file: {0}")]
    LoadError(std::io::Error),
    /// Parse error. The error are printed in the stderr, and also are in the vector
    #[error("{0:?}")]
    CompileError(Vec<String>),
    /// Cannot write the generated file
    #[error("Cannot load the .60 file: {0}")]
    SaveError(std::io::Error),
}

/// Compile the `.60` file and generate rust code for it.
///
/// The path is relative to the `CARGO_MANIFEST_DIR`.
///
/// The following line need to be added within your crate to include the generated code.
/// ```ignore
/// sixtyfps::include_modules!();
/// ```
pub fn compile(path: impl AsRef<std::path::Path>) -> Result<(), CompileError> {
    let path = Path::new(&env::var_os("CARGO_MANIFEST_DIR").ok_or(CompileError::NotRunViaCargo)?)
        .join(path.as_ref());

    let source = std::fs::read_to_string(&path).map_err(CompileError::LoadError)?;
    let (syntax_node, mut diag) = parser::parse(&source);
    diag.current_path = path.clone();

    if diag.has_error() {
        let vec = diag.inner.iter().map(|d| d.message.clone()).collect();
        diag.print(source);
        return Err(CompileError::CompileError(vec));
    }

    let mut tr = typeregister::TypeRegister::builtin();
    let doc = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
    let mut compiler_config = CompilerConfiguration::default();

    if let Some(target) = env::var("TARGET").ok() {
        if target == "wasm32-unknown-unknown" {
            compiler_config.embed_resources = true;
        }
    };

    run_passes(&doc, &mut diag, &mut tr, &compiler_config);

    if diag.has_error() {
        let vec = diag.inner.iter().map(|d| d.message.clone()).collect();
        diag.print(source);
        return Err(CompileError::CompileError(vec));
    }

    let output_file_path = Path::new(&env::var_os("OUT_DIR").ok_or(CompileError::NotRunViaCargo)?)
        .join(
            path.file_stem()
                .map(Path::new)
                .unwrap_or(Path::new("sixtyfps_out"))
                .with_extension("rs"),
        );

    let mut file = std::fs::File::create(&output_file_path).map_err(CompileError::SaveError)?;
    let generated = generator::rust::generate(&doc.root_component, &mut diag).ok_or_else(|| {
        let vec = diag.inner.iter().map(|d| d.message.clone()).collect();
        diag.print(source);
        CompileError::CompileError(vec)
    })?;
    write!(file, "{}", generated).map_err(CompileError::SaveError)?;
    println!("cargo:rerun-if-changed={}", path.display());
    println!("cargo:rustc-env=SIXTYFPS_INCLUDE_GENERATED={}", output_file_path.display());
    Ok(())
}
