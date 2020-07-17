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

struct CodeFormatter<Sink> {
    indentation: usize,
    in_string: bool,
    sink: Sink,
}

impl<Sink: Write> Write for CodeFormatter<Sink> {
    fn write(&mut self, mut s: &[u8]) -> std::io::Result<usize> {
        let len = s.len();
        while let Some(idx) = s.iter().position(|c| match c {
            b'{' if !self.in_string => {
                self.indentation += 1;
                true
            }
            b'}' if !self.in_string => {
                self.indentation -= 1;
                true
            }
            b';' if !self.in_string => true,
            b'"' if !self.in_string => {
                self.in_string = true;
                false
            }
            b'"' if self.in_string => {
                // FIXME! escape character
                self.in_string = false;
                false
            }
            _ => false,
        }) {
            let idx = idx + 1;
            self.sink.write_all(&s[..idx])?;
            self.sink.write_all(b"\n")?;
            for _ in 0..self.indentation {
                self.sink.write_all(b"    ")?;
            }
            s = &s[idx..];
        }
        self.sink.write_all(s)?;
        Ok(len)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }
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

    let (syntax_node, diag) = parser::parse_file(&path).map_err(CompileError::LoadError)?;

    if diag.has_error() {
        let vec = diag.to_string_vec();
        diag.print();
        return Err(CompileError::CompileError(vec));
    }

    let mut compiler_config = CompilerConfiguration::default();

    if let Some(target) = env::var("TARGET").ok() {
        if target == "wasm32-unknown-unknown" {
            compiler_config.embed_resources = true;
        }
    };

    let (doc, mut diag) = compile_syntax_node(syntax_node, diag, &compiler_config);

    if diag.has_error() {
        let vec = diag.to_string_vec();
        diag.print();
        return Err(CompileError::CompileError(vec));
    }

    let output_file_path = Path::new(&env::var_os("OUT_DIR").ok_or(CompileError::NotRunViaCargo)?)
        .join(
            path.file_stem()
                .map(Path::new)
                .unwrap_or(Path::new("sixtyfps_out"))
                .with_extension("rs"),
        );

    let file = std::fs::File::create(&output_file_path).map_err(CompileError::SaveError)?;
    let mut code_formater = CodeFormatter { indentation: 0, in_string: false, sink: file };
    let generated = generator::rust::generate(&doc.root_component, &mut diag).ok_or_else(|| {
        let vec = diag.to_string_vec();
        diag.print();
        CompileError::CompileError(vec)
    })?;
    write!(code_formater, "{}", generated).map_err(CompileError::SaveError)?;
    println!("cargo:rerun-if-changed={}", path.display());
    println!("cargo:rustc-env=SIXTYFPS_INCLUDE_GENERATED={}", output_file_path.display());
    Ok(())
}
