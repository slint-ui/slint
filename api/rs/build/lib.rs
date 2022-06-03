// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This crate serves as a companion crate of the slint crate.
It is meant to allow you to compile the `.slint` files from your `build.rs` script.

The main entry point of this crate is the [`compile()`] function

## Example

In your Cargo.toml:

```toml
[package]
...
build = "build.rs"

[dependencies]
slint = "0.2.4"
...

[build-dependencies]
slint-build = "0.2.4"
```

In the `build.rs` file:

```ignore
fn main() {
    slint_build::compile("ui/hello.slint").unwrap();
}
```

Then in your main file

```ignore
slint::include_modules!();
fn main() {
    HelloWorld::new().run();
}
```
*/
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
#![warn(missing_docs)]

#[cfg(not(feature = "default"))]
compile_error!(
    "The feature `default` must be enabled to ensure \
    forward compatibility with future version of this crate"
);

use std::env;
use std::io::Write;
use std::path::Path;

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::EmbedResourcesKind;

/// The structure for configuring aspects of the compilation of `.slint` markup files to Rust.
pub struct CompilerConfiguration {
    config: i_slint_compiler::CompilerConfiguration,
}

impl Default for CompilerConfiguration {
    fn default() -> Self {
        Self {
            config: i_slint_compiler::CompilerConfiguration::new(
                i_slint_compiler::generator::OutputFormat::Rust,
            ),
        }
    }
}

impl CompilerConfiguration {
    /// Creates a new default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new configuration that includes sets the include paths used for looking up
    /// `.slint` imports to the specified vector of paths.
    #[must_use]
    pub fn with_include_paths(self, include_paths: Vec<std::path::PathBuf>) -> Self {
        let mut config = self.config;
        config.include_paths = include_paths;
        Self { config }
    }

    /// Create a new configuration that selects the style to be used for widgets.
    #[must_use]
    pub fn with_style(self, style: String) -> Self {
        let mut config = self.config;
        config.style = Some(style);
        Self { config }
    }
}

/// Error returned by the `compile` function
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum CompileError {
    /// Cannot read environment variable CARGO_MANIFEST_DIR or OUT_DIR. The build script need to be run via cargo.
    #[error("Cannot read environment variable CARGO_MANIFEST_DIR or OUT_DIR. The build script need to be run via cargo.")]
    NotRunViaCargo,
    /// Parse error. The error are printed in the stderr, and also are in the vector
    #[error("{0:?}")]
    CompileError(Vec<String>),
    /// Cannot write the generated file
    #[error("Cannot write the generated file: {0}")]
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

/// Compile the `.slint` file and generate rust code for it.
///
/// The generated code code will be created in the directory specified by
/// the `OUT` environment variable as it is expected for build script.
///
/// The following line need to be added within your crate in order to include
/// the generated code.
/// ```ignore
/// slint::include_modules!();
/// ```
///
/// The path is relative to the `CARGO_MANIFEST_DIR`.
///
/// In case of compilation error, the errors are shown in `stderr`, the error
/// are also returned in the [`CompileError`] enum. You must `unwrap` the returned
/// result to make sure that cargo make the compilation fail in case there were
/// errors when generating the code.
///
/// Please check out the documentation of the `slint` crate for more information
/// about how to use the generated code.
pub fn compile(path: impl AsRef<std::path::Path>) -> Result<(), CompileError> {
    compile_with_config(path, CompilerConfiguration::default())
}

/// Same as [`compile`], but allow to specify a configuration.
pub fn compile_with_config(
    path: impl AsRef<std::path::Path>,
    config: CompilerConfiguration,
) -> Result<(), CompileError> {
    let path = Path::new(&env::var_os("CARGO_MANIFEST_DIR").ok_or(CompileError::NotRunViaCargo)?)
        .join(path.as_ref());

    let mut diag = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse_file(&path, &mut diag);

    if diag.has_error() {
        let vec = diag.to_string_vec();
        diag.print();
        return Err(CompileError::CompileError(vec));
    }

    let mut compiler_config = config.config;

    if env::var_os("DEP_I_SLINT_BACKEND_MCU_EMBED_TEXTURES").is_some() {
        compiler_config.embed_resources = EmbedResourcesKind::EmbedTextures;
        // TODO: generalize by either having dedicated variable in mcu backend to disable a11y or
        // use separate if block to disable features like this when any DEP_I_SLINT_BACKEND_MCU is set
        compiler_config.accessibility = false;
    } else if let (Ok(target), Ok(host)) = (env::var("TARGET"), env::var("HOST")) {
        if target != host {
            compiler_config.embed_resources = EmbedResourcesKind::EmbedAllResources;
        }
    };
    let mut rerun_if_changed = String::new();

    if std::env::var_os("SLINT_STYLE").is_none()
        && std::env::var_os("SIXTYFPS_STYLE").is_none()
        && compiler_config.style.is_none()
    {
        compiler_config.style = std::env::var_os("OUT_DIR").and_then(|path| {
            // Same logic as in i-slint-backend-selector's build script to get the path
            let path = Path::new(&path).parent()?.parent()?.join("SLINT_DEFAULT_STYLE.txt");
            // unfortunately, if for some reason the file is changed by the i-slint-backend-selector's build script,
            // it is changed after cargo decide to re-run this build script or not. So that means one will need two build
            // to settle the right thing.
            rerun_if_changed = format!("cargo:rerun-if-changed={}", path.display());
            let style = std::fs::read_to_string(path).ok()?;
            Some(style.trim().into())
        });
    }

    let syntax_node = syntax_node.expect("diags contained no compilation errors");

    // 'spin_on' is ok here because the compiler in single threaded and does not block if there is no blocking future
    let (doc, diag) =
        spin_on::spin_on(i_slint_compiler::compile_syntax_node(syntax_node, diag, compiler_config));

    if diag.has_error() {
        let vec = diag.to_string_vec();
        diag.print();
        return Err(CompileError::CompileError(vec));
    }

    let output_file_path = Path::new(&env::var_os("OUT_DIR").ok_or(CompileError::NotRunViaCargo)?)
        .join(
            path.file_stem()
                .map(Path::new)
                .unwrap_or_else(|| Path::new("slint_out"))
                .with_extension("rs"),
        );

    let file = std::fs::File::create(&output_file_path).map_err(CompileError::SaveError)?;
    let mut code_formatter = CodeFormatter { indentation: 0, in_string: false, sink: file };
    let generated = i_slint_compiler::generator::rust::generate(&doc);

    for x in &diag.all_loaded_files {
        if x.is_absolute() {
            println!("cargo:rerun-if-changed={}", x.display());
        }
    }

    // print warnings
    diag.diagnostics_as_string().lines().for_each(|w| {
        if !w.is_empty() {
            println!("cargo:warning={}", w.strip_prefix("warning: ").unwrap_or(w))
        }
    });

    write!(code_formatter, "{}", generated).map_err(CompileError::SaveError)?;
    println!("{}\ncargo:rerun-if-changed={}", rerun_if_changed, path.display());

    for resource in doc.root_component.embedded_file_resources.borrow().keys() {
        if !resource.starts_with("builtin:") {
            println!("cargo:rerun-if-changed={}", resource);
        }
    }
    println!("cargo:rerun-if-env-changed=SLINT_STYLE");
    println!("cargo:rerun-if-env-changed=SIXTYFPS_STYLE");
    println!("cargo:rerun-if-env-changed=SLINT_FONT_SIZES");
    println!("cargo:rerun-if-env-changed=SLINT_SCALE_FACTOR");

    println!("cargo:rustc-env=SLINT_INCLUDE_GENERATED={}", output_file_path.display());

    Ok(())
}

/// This function is for use the application's build script, in order to print any device specific
/// build flags reported by the backend
pub fn print_rustc_flags() -> std::io::Result<()> {
    if let Some(board_config_path) =
        std::env::var_os("DEP_I_SLINT_BACKEND_MCU_BOARD_CONFIG_PATH").map(std::path::PathBuf::from)
    {
        let config = std::fs::read_to_string(board_config_path.as_path())?;
        let toml = config.parse::<toml_edit::Document>().expect("invalid board config toml");

        for link_arg in
            toml.get("link_args").and_then(toml_edit::Item::as_array).into_iter().flatten()
        {
            if let Some(option) = link_arg.as_str() {
                println!("cargo:rustc-link-arg={}", option);
            }
        }

        for link_search_path in
            toml.get("link_search_path").and_then(toml_edit::Item::as_array).into_iter().flatten()
        {
            if let Some(mut path) = link_search_path.as_str().map(std::path::PathBuf::from) {
                if path.is_relative() {
                    path = board_config_path.parent().unwrap().join(path);
                }
                println!("cargo:rustc-link-search={}", path.to_string_lossy());
            }
        }
        println!("cargo:rerun-if-env-changed=DEP_I_SLINT_BACKEND_MCU_BOARD_CONFIG_PATH");
        println!("cargo:rerun-if-changed={}", board_config_path.display());
    }

    Ok(())
}
