// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
slint = "1.9.0"
...

[build-dependencies]
slint-build = "1.9.0"
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
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![warn(missing_docs)]

#[cfg(not(feature = "default"))]
compile_error!(
    "The feature `default` must be enabled to ensure \
    forward compatibility with future version of this crate"
);

use std::collections::HashMap;
use std::env;
use std::io::{BufWriter, Write};
use std::path::Path;

use i_slint_compiler::diagnostics::BuildDiagnostics;

/// The structure for configuring aspects of the compilation of `.slint` markup files to Rust.
pub struct CompilerConfiguration {
    config: i_slint_compiler::CompilerConfiguration,
}

/// How should the slint compiler embed images and fonts
///
/// Parameter of [`CompilerConfiguration::embed_resources()`]
#[derive(Clone, PartialEq)]
pub enum EmbedResourcesKind {
    /// Paths specified in .slint files are made absolute and the absolute
    /// paths will be used at run-time to load the resources from the file system.
    AsAbsolutePath,
    /// The raw files in .slint files are embedded in the application binary.
    EmbedFiles,
    /// File names specified in .slint files will be loaded by the Slint compiler,
    /// optimized for use with the software renderer and embedded in the application binary.
    EmbedForSoftwareRenderer,
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

    /// Create a new configuration that sets the library paths used for looking up
    /// `@library` imports to the specified map of paths.
    ///
    /// Each library path can either be a path to a `.slint` file or a directory.
    /// If it's a file, the library is imported by its name prefixed by `@` (e.g.
    /// `@example`). The specified file is the only entry-point for the library
    /// and other files from the library won't be accessible from the outside.
    /// If it's a directory, a specific file in that directory must be specified
    /// when importing the library (e.g. `@example/widgets.slint`). This allows
    /// exposing multiple entry-points for a single library.
    ///
    /// Compile `ui/main.slint` and specify an "example" library path:
    /// ```rust,no_run
    /// let manifest_dir = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    /// let library_paths = std::collections::HashMap::from([(
    ///     "example".to_string(),
    ///     manifest_dir.join("third_party/example/ui/lib.slint"),
    /// )]);
    /// let config = slint_build::CompilerConfiguration::new().with_library_paths(library_paths);
    /// slint_build::compile_with_config("ui/main.slint", config).unwrap();
    /// ```
    ///
    /// Import the "example" library in `ui/main.slint`:
    /// ```slint,ignore
    /// import { Example } from "@example";
    /// ```
    #[must_use]
    pub fn with_library_paths(self, library_paths: HashMap<String, std::path::PathBuf>) -> Self {
        let mut config = self.config;
        config.library_paths = library_paths;
        Self { config }
    }

    /// Create a new configuration that selects the style to be used for widgets.
    #[must_use]
    pub fn with_style(self, style: String) -> Self {
        let mut config = self.config;
        config.style = Some(style);
        Self { config }
    }

    /// Selects how the resources such as images and font are processed.
    ///
    /// See [`EmbedResourcesKind`]
    #[must_use]
    pub fn embed_resources(self, kind: EmbedResourcesKind) -> Self {
        let mut config = self.config;
        config.embed_resources = match kind {
            EmbedResourcesKind::AsAbsolutePath => {
                i_slint_compiler::EmbedResourcesKind::OnlyBuiltinResources
            }
            EmbedResourcesKind::EmbedFiles => {
                i_slint_compiler::EmbedResourcesKind::EmbedAllResources
            }
            EmbedResourcesKind::EmbedForSoftwareRenderer => {
                i_slint_compiler::EmbedResourcesKind::EmbedTextures
            }
        };
        Self { config }
    }

    /// Sets the scale factor to be applied to all `px` to `phx` conversions
    /// as constant value. This is only intended for MCU environments. Use
    /// in combination with [`Self::embed_resources`] to pre-scale images and glyphs
    /// accordingly.
    #[must_use]
    pub fn with_scale_factor(self, factor: f32) -> Self {
        let mut config = self.config;
        config.const_scale_factor = factor as f64;
        Self { config }
    }

    /// Configures the compiler to bundle translations when compiling Slint code.
    ///
    /// It expects the path to be the root directory of the translation files.
    ///
    /// The translation files should be in the gettext `.po` format and follow this pattern:
    /// `<path>/<lang>/LC_MESSAGES/<crate>.po`
    #[must_use]
    pub fn with_bundled_translations(
        self,
        path: impl Into<std::path::PathBuf>,
    ) -> CompilerConfiguration {
        let mut config = self.config;
        config.translation_path_bundle = Some(path.into());
        Self { config }
    }
}

/// Error returned by the `compile` function
#[derive(derive_more::Error, derive_more::Display, Debug)]
#[non_exhaustive]
pub enum CompileError {
    /// Cannot read environment variable CARGO_MANIFEST_DIR or OUT_DIR. The build script need to be run via cargo.
    #[display("Cannot read environment variable CARGO_MANIFEST_DIR or OUT_DIR. The build script need to be run via cargo.")]
    NotRunViaCargo,
    /// Parse error. The error are printed in the stderr, and also are in the vector
    #[display("{_0:?}")]
    CompileError(#[error(not(source))] Vec<String>),
    /// Cannot write the generated file
    #[display("Cannot write the generated file: {_0}")]
    SaveError(std::io::Error),
}

struct CodeFormatter<Sink> {
    indentation: usize,
    /// We are currently in a string
    in_string: bool,
    /// number of bytes after the last `'`, 0 if there was none
    in_char: usize,
    /// In string or char, and the previous character was `\\`
    escaped: bool,
    sink: Sink,
}

impl<Sink> CodeFormatter<Sink> {
    pub fn new(sink: Sink) -> Self {
        Self { indentation: 0, in_string: false, in_char: 0, escaped: false, sink }
    }
}

impl<Sink: Write> Write for CodeFormatter<Sink> {
    fn write(&mut self, mut s: &[u8]) -> std::io::Result<usize> {
        let len = s.len();
        while let Some(idx) = s.iter().position(|c| match c {
            b'{' if !self.in_string && self.in_char == 0 => {
                self.indentation += 1;
                true
            }
            b'}' if !self.in_string && self.in_char == 0 => {
                self.indentation -= 1;
                true
            }
            b';' if !self.in_string && self.in_char == 0 => true,
            b'"' if !self.in_string && self.in_char == 0 => {
                self.in_string = true;
                self.escaped = false;
                false
            }
            b'"' if self.in_string && !self.escaped => {
                self.in_string = false;
                false
            }
            b'\'' if !self.in_string && self.in_char == 0 => {
                self.in_char = 1;
                self.escaped = false;
                false
            }
            b'\'' if !self.in_string && self.in_char > 0 && !self.escaped => {
                self.in_char = 0;
                false
            }
            b' ' | b'>' if self.in_char > 2 && !self.escaped => {
                // probably a lifetime
                self.in_char = 0;
                false
            }
            b'\\' if (self.in_string || self.in_char > 0) && !self.escaped => {
                self.escaped = true;
                // no need to increment in_char since \ isn't a single character
                false
            }
            _ if self.in_char > 0 => {
                self.in_char += 1;
                self.escaped = false;
                false
            }
            _ => {
                self.escaped = false;
                false
            }
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

#[test]
fn formatter_test() {
    fn format_code(code: &str) -> String {
        let mut res = Vec::new();
        let mut formatter = CodeFormatter::new(&mut res);
        formatter.write_all(code.as_bytes()).unwrap();
        String::from_utf8(res).unwrap()
    }

    assert_eq!(
        format_code("fn main() { if ';' == '}' { return \";\"; } else { panic!() } }"),
        r#"fn main() {
     if ';' == '}' {
         return ";";
         }
     else {
         panic!() }
     }
"#
    );

    assert_eq!(
        format_code(r#"fn xx<'lt>(foo: &'lt str) { println!("{}", '\u{f700}'); return Ok(()); }"#),
        r#"fn xx<'lt>(foo: &'lt str) {
     println!("{}", '\u{f700}');
     return Ok(());
     }
"#
    );

    assert_eq!(
        format_code(r#"fn main() { ""; "'"; "\""; "{}"; "\\"; "\\\""; }"#),
        r#"fn main() {
     "";
     "'";
     "\"";
     "{}";
     "\\";
     "\\\"";
     }
"#
    );

    assert_eq!(
        format_code(r#"fn main() { '"'; '\''; '{'; '}'; '\\'; }"#),
        r#"fn main() {
     '"';
     '\'';
     '{';
     '}';
     '\\';
     }
"#
    );
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
///
/// This function can only be called within a build script run by cargo.
pub fn compile(path: impl AsRef<std::path::Path>) -> Result<(), CompileError> {
    compile_with_config(path, CompilerConfiguration::default())
}

/// Same as [`compile`], but allow to specify a configuration.
///
/// Compile `ui/hello.slint` and select the "material" style:
/// ```rust,no_run
/// let config =
///     slint_build::CompilerConfiguration::new()
///     .with_style("material".into());
/// slint_build::compile_with_config("ui/hello.slint", config).unwrap();
/// ```
pub fn compile_with_config(
    relative_slint_file_path: impl AsRef<std::path::Path>,
    config: CompilerConfiguration,
) -> Result<(), CompileError> {
    let path = Path::new(&env::var_os("CARGO_MANIFEST_DIR").ok_or(CompileError::NotRunViaCargo)?)
        .join(relative_slint_file_path.as_ref());

    let absolute_rust_output_file_path =
        Path::new(&env::var_os("OUT_DIR").ok_or(CompileError::NotRunViaCargo)?).join(
            path.file_stem()
                .map(Path::new)
                .unwrap_or_else(|| Path::new("slint_out"))
                .with_extension("rs"),
        );

    let paths_dependencies =
        compile_with_output_path(path, absolute_rust_output_file_path.clone(), config)?;

    for path_dependency in paths_dependencies {
        println!("cargo:rerun-if-changed={}", path_dependency.display());
    }

    println!("cargo:rerun-if-env-changed=SLINT_STYLE");
    println!("cargo:rerun-if-env-changed=SLINT_FONT_SIZES");
    println!("cargo:rerun-if-env-changed=SLINT_SCALE_FACTOR");
    println!("cargo:rerun-if-env-changed=SLINT_ASSET_SECTION");
    println!("cargo:rerun-if-env-changed=SLINT_EMBED_RESOURCES");
    println!("cargo:rerun-if-env-changed=SLINT_EMIT_DEBUG_INFO");

    println!(
        "cargo:rustc-env=SLINT_INCLUDE_GENERATED={}",
        absolute_rust_output_file_path.display()
    );

    Ok(())
}

/// Similar to [`compile_with_config`], but meant to be used independently of cargo.
///
/// Will compile the input file and write the result in the given output file.
///
/// Both input_slint_file_path and output_rust_file_path should be absolute paths.
///
/// Doesn't print any cargo messages.
///
/// Returns a list of all input files that were used to generate the output file. (dependencies)
pub fn compile_with_output_path(
    input_slint_file_path: impl AsRef<std::path::Path>,
    output_rust_file_path: impl AsRef<std::path::Path>,
    config: CompilerConfiguration,
) -> Result<Vec<std::path::PathBuf>, CompileError> {
    let mut diag = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse_file(&input_slint_file_path, &mut diag);

    if diag.has_errors() {
        let vec = diag.to_string_vec();
        diag.print();
        return Err(CompileError::CompileError(vec));
    }

    let mut compiler_config = config.config;
    compiler_config.translation_domain = std::env::var("CARGO_PKG_NAME").ok();

    let syntax_node = syntax_node.expect("diags contained no compilation errors");

    // 'spin_on' is ok here because the compiler in single threaded and does not block if there is no blocking future
    let (doc, diag, loader) =
        spin_on::spin_on(i_slint_compiler::compile_syntax_node(syntax_node, diag, compiler_config));

    if diag.has_errors() {
        let vec = diag.to_string_vec();
        diag.print();
        return Err(CompileError::CompileError(vec));
    }

    let output_file =
        std::fs::File::create(&output_rust_file_path).map_err(CompileError::SaveError)?;
    let mut code_formatter = CodeFormatter::new(BufWriter::new(output_file));
    let generated = i_slint_compiler::generator::rust::generate(&doc, &loader.compiler_config)
        .map_err(|e| CompileError::CompileError(vec![e.to_string()]))?;

    let mut dependencies: Vec<std::path::PathBuf> = Vec::new();

    for x in &diag.all_loaded_files {
        if x.is_absolute() {
            dependencies.push(x.clone());
        }
    }

    // print warnings
    diag.diagnostics_as_string().lines().for_each(|w| {
        if !w.is_empty() {
            println!("cargo:warning={}", w.strip_prefix("warning: ").unwrap_or(w))
        }
    });

    write!(code_formatter, "{}", generated).map_err(CompileError::SaveError)?;
    dependencies.push(input_slint_file_path.as_ref().to_path_buf());

    for resource in doc.embedded_file_resources.borrow().keys() {
        if !resource.starts_with("builtin:") {
            dependencies.push(Path::new(resource).to_path_buf());
        }
    }

    Ok(dependencies)
}

/// This function is for use the application's build script, in order to print any device specific
/// build flags reported by the backend
pub fn print_rustc_flags() -> std::io::Result<()> {
    if let Some(board_config_path) =
        std::env::var_os("DEP_MCU_BOARD_SUPPORT_BOARD_CONFIG_PATH").map(std::path::PathBuf::from)
    {
        let config = std::fs::read_to_string(board_config_path.as_path())?;
        let toml = config.parse::<toml_edit::DocumentMut>().expect("invalid board config toml");

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
        println!("cargo:rerun-if-env-changed=DEP_MCU_BOARD_SUPPORT_MCU_BOARD_CONFIG_PATH");
        println!("cargo:rerun-if-changed={}", board_config_path.display());
    }

    Ok(())
}
