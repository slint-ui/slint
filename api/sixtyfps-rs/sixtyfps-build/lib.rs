/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
This crate serves as a companion crate for the sixtyfps crate.
It is meant to allow you to compile the `.60` files from your `build.rs`script.

The main entry point of this crate is the [`compile()`] function

## Example

In your Cargo.toml:

```toml
[package]
...
build = "build.rs"

[dependencies]
sixtyfps = "0.0.2"
...

[build-dependencies]
sixtyfps-build = "0.0.2"
```

In the `build.rs` file:

```ignore
fn main() {
    sixtyfps_build::compile("ui/hello.60").unwrap();
}
```

Then in your main file

```ignore
sixtyfps::include_modules!();
fn main() {
    HelloWorld::new().run()
}
```
*/

#![warn(missing_docs)]

use std::env;
use std::io::Write;
use std::path::Path;

pub use sixtyfps_compilerlib::CompilerConfiguration;

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
/// The generated code code will be created in the directory specified by
/// the `OUT` environment variable as it is expected for build script.
///
/// The following line need to be added within your crate in order to include
/// the generated code.
/// ```ignore
/// sixtyfps::include_modules!();
/// ```
///
/// The path is relative to the `CARGO_MANIFEST_DIR`.
///
/// In case of compilation error, the errors are shown in `stderr`, the error
/// are also returned in the [`CompileError`] enum. You must `unwrap` the returned
/// result to make sure that cargo make the compilation fail in case there were
/// errors when generating the code.
///
/// Please check out the documentation of the `sixtyfps` crate for more information
/// about how to use the generated code.
pub fn compile(path: impl AsRef<std::path::Path>) -> Result<(), CompileError> {
    compile_with_config(path, Default::default())
}

/// Same as [`compile`], but allow to specify a configuration
pub fn compile_with_config(
    path: impl AsRef<std::path::Path>,
    config: CompilerConfiguration,
) -> Result<(), CompileError> {
    let path = Path::new(&env::var_os("CARGO_MANIFEST_DIR").ok_or(CompileError::NotRunViaCargo)?)
        .join(path.as_ref());

    let (syntax_node, diag) =
        sixtyfps_compilerlib::parser::parse_file(&path).map_err(CompileError::LoadError)?;

    if diag.has_error() {
        let vec = diag.to_string_vec();
        diag.print();
        return Err(CompileError::CompileError(vec));
    }

    let mut compiler_config = config;

    if let (Ok(target), Ok(host)) = (env::var("TARGET"), env::var("HOST")) {
        if target != host {
            compiler_config.embed_resources = true;
        }
    };

    let (doc, mut diag) =
        sixtyfps_compilerlib::compile_syntax_node(syntax_node, diag, &compiler_config);

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
    let generated = match sixtyfps_compilerlib::generator::rust::generate(&doc, &mut diag) {
        Some(code) => {
            // print warnings
            diag.diagnostics_as_string().lines().for_each(|w| {
                if !w.is_empty() {
                    println!("cargo:warning={}", w.strip_prefix("warning: ").unwrap_or(w))
                }
            });
            code
        }
        None => {
            let vec = diag.to_string_vec();
            diag.print();
            return Err(CompileError::CompileError(vec));
        }
    };

    write!(code_formater, "{}", generated).map_err(CompileError::SaveError)?;
    println!("cargo:rerun-if-changed={}", path.display());

    if !compiler_config.embed_resources {
        for resource in doc.root_component.referenced_file_resources.borrow().keys() {
            println!("cargo:rerun-if-changed={}", resource);
        }
    }
    println!("cargo:rerun-if-env-changed=SIXTYFPS_STYLE");

    println!("cargo:rustc-env=SIXTYFPS_INCLUDE_GENERATED={}", output_file_path.display());
    Ok(())
}
