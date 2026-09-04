// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This crate serves as a companion crate of the slint crate.
It is meant to allow you to compile the `.slint` files from your `build.rs` script.

The main entry point of this crate is the [`compile()`] function

The generated code must be included in your crate by using the `slint::include_modules!()` macro.

## Example

In your Cargo.toml:

```toml
[package]
...
build = "build.rs"

[dependencies]
slint = "1.16.0"
...

[build-dependencies]
slint-build = "1.16.0"
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
#![cfg_attr(
    feature = "document-features",
    doc = concat!("## Feature flags\n\n", document_features::document_features!())
)]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![warn(missing_docs)]

#[cfg(not(feature = "compat-1-18"))]
compile_error!(
    "The feature `compat-1-18` must be enabled to ensure \
    forward compatibility with future version of this crate"
);

use std::collections::HashMap;
use std::env;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::project_file::{FILE_NAME as PROJECT_FILE_NAME, ProjectFile};

/// Argument of [`CompilerConfiguration::with_default_translation_context()`]
///
pub use i_slint_compiler::DefaultTranslationContext;

/// The structure for configuring aspects of the compilation of `.slint` markup files to Rust.
#[derive(Clone)]
pub struct CompilerConfiguration {
    config: i_slint_compiler::CompilerConfiguration,
    project_file: Option<ProjectFile>,
}

/// How should the Slint compiler embed images and fonts
///
/// Parameter of [`CompilerConfiguration::embed_resources()`]
#[derive(Clone, PartialEq)]
pub enum EmbedResourcesKind {
    /// Resources are loaded from their absolute path at run-time.
    ///
    /// Only useful for debugging, since the files must still be present at the same path on the
    /// machine running the application.
    AsAbsolutePath,
    /// The files referenced from .slint files are embedded in the binary as-is (for example
    /// a PNG stays compressed), and decoded at run-time.
    EmbedFiles,
    #[cfg(feature = "renderer-software")]
    /// Images and fonts are pre-processed at compile time and embedded as uncompressed pixel
    /// data, ready to be drawn by the software renderer without any decoding at run-time.
    ///
    /// Useful for MCUs with no file system and little RAM.
    /// Only the Slint software renderer can use these resources; Skia and FemtoVG can't.
    EmbedForSoftwareRenderer,
}

impl Default for CompilerConfiguration {
    /// Create a CompilerConfiguration based on the project file (slint.project.json).
    ///
    /// All settings that are empty in the project file are set to their default values.
    ///
    /// # Panics
    ///
    /// This function panics if the project file exists but is invalid.
    fn default() -> Self {
        let project_file = discover_project_file()
            .unwrap_or_else(|err| panic!("Failed to load {PROJECT_FILE_NAME}: {err}"));

        let config = project_file
            .as_ref()
            .map(|project_file| {
                project_file
                    .into_compiler_configuration(i_slint_compiler::generator::OutputFormat::Rust)
            })
            .unwrap_or_else(|| {
                i_slint_compiler::CompilerConfiguration::new(
                    i_slint_compiler::generator::OutputFormat::Rust,
                )
            });

        Self { project_file, config }
    }
}

impl CompilerConfiguration {
    /// Creates a new configuration, based on the project file (slint.project.json).
    ///
    /// All settings that are empty in the project file are set to their default values.
    ///
    /// # Panics
    ///
    /// This function panics if the project file exists but is invalid.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new configuration that includes sets the include paths used for looking up
    /// `.slint` imports to the specified vector of paths.
    #[must_use]
    pub fn with_include_paths(self, include_paths: Vec<std::path::PathBuf>) -> Self {
        let mut this = self;
        this.config.include_paths = include_paths;
        this
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
    pub fn with_library_paths(
        mut self,
        library_paths: HashMap<String, std::path::PathBuf>,
    ) -> Self {
        self.config.library_paths = library_paths;
        self
    }

    /// Create a new configuration that selects the style to be used for widgets.
    #[must_use]
    pub fn with_style(mut self, style: String) -> Self {
        self.config.style = Some(style);
        self
    }

    /// Selects how the resources such as images and font are processed.
    ///
    /// See [`EmbedResourcesKind`]
    #[must_use]
    pub fn embed_resources(mut self, kind: EmbedResourcesKind) -> Self {
        self.config.embed_resources = match kind {
            EmbedResourcesKind::AsAbsolutePath => {
                i_slint_compiler::EmbedResourcesKind::OnlyBuiltinResources
            }
            EmbedResourcesKind::EmbedFiles => {
                i_slint_compiler::EmbedResourcesKind::EmbedAllResources
            }
            #[cfg(feature = "renderer-software")]
            EmbedResourcesKind::EmbedForSoftwareRenderer => {
                i_slint_compiler::EmbedResourcesKind::EmbedTextures
            }
        };
        self
    }

    /// Sets the scale factor to be applied to all `px` to `phx` conversions
    /// as constant value. This is only intended for MCU environments. Use
    /// in combination with [`Self::embed_resources`] to pre-scale images and glyphs
    /// accordingly.
    ///
    /// If this is set, changing the scale factor at runtime will not have any effect.
    #[must_use]
    pub fn with_scale_factor(mut self, factor: f32) -> Self {
        self.config.const_scale_factor = Some(factor);
        self
    }

    /// Configures the compiler to bundle translations when compiling Slint code.
    ///
    /// It expects the path to be the root directory of the translation files.
    ///
    /// If given a relative path, it will be resolved relative to `$CARGO_MANIFEST_DIR`.
    ///
    /// The translation files should be in the gettext `.po` format and follow this pattern:
    /// `<path>/<lang>/LC_MESSAGES/<crate>.po`
    #[must_use]
    pub fn with_bundled_translations(
        mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> CompilerConfiguration {
        self.config.translation_path_bundle = Some(path.into());
        self
    }

    /// Unless explicitly specified with the `@tr("context" => ...)`, the default translation context is the component name.
    /// Use this option with [`DefaultTranslationContext::None`] to disable the default translation context.
    ///
    /// The translation file must also not have context
    /// (`--no-default-translation-context` argument of `slint-tr-extractor`)
    #[must_use]
    pub fn with_default_translation_context(
        mut self,
        default_translation_context: DefaultTranslationContext,
    ) -> Self {
        self.config.default_translation_context = default_translation_context;
        self
    }

    /// Configures the compiler to emit additional debug info when compiling Slint code.
    ///
    /// This is the equivalent to setting `SLINT_EMIT_DEBUG_INFO=1` and using the `slint!()` macro
    /// and is primarily used by `i-slint-backend-testing`.
    #[doc(hidden)]
    #[must_use]
    pub fn with_debug_info(mut self, enable: bool) -> Self {
        self.config.debug_info = enable;
        self
    }

    /// Configures the compiler to treat the Slint as part of a library.
    ///
    /// Use this when the components and types of the Slint code need
    /// to be accessible from other modules.
    ///
    /// **Note**: This feature is experimental and may change or be removed in the future.
    #[cfg(feature = "experimental-module-builds")]
    #[must_use]
    pub fn as_library(self, library_name: &str) -> Self {
        let mut this = self;
        let mut config = this.config;
        config.library_name = Some(library_name.to_string());
        this.config = config;
        this
    }

    /// Specify the Rust module to place the generated code in.
    ///
    /// **Note**: This feature is experimental and may change or be removed in the future.
    #[cfg(feature = "experimental-module-builds")]
    #[must_use]
    pub fn rust_module(mut self, rust_module: &str) -> Self {
        self.config.rust_module = Some(rust_module.to_string());
        self
    }
    /// Configures the compiler to use Signed Distance Field (SDF) encoding for fonts.
    ///
    /// This flag only takes effect when `embed_resources` is set to [`EmbedResourcesKind::EmbedForSoftwareRenderer`],
    /// and requires the `sdf-fonts` cargo feature to be enabled.
    ///
    /// [SDF](https://en.wikipedia.org/wiki/Signed_distance_function) reduces the binary size by
    /// using an alternative representation for fonts, trading off some rendering quality
    /// for a smaller binary footprint.
    /// Rendering is slower and may result in slightly inferior visual output.
    /// Use this on systems with limited flash memory.
    #[cfg(feature = "sdf-fonts")]
    #[must_use]
    pub fn with_sdf_fonts(mut self, enable: bool) -> Self {
        self.config.use_sdf_fonts = enable;
        self
    }

    /// Converts any relative include_paths or library_paths to absolute paths relative to the manifest_dir.
    #[must_use]
    fn with_absolute_paths(self, manifest_dir: &std::path::Path) -> Self {
        let Self { mut config, project_file } = self;

        let to_absolute_path = |path: &mut std::path::PathBuf| {
            if path.is_relative() {
                *path = manifest_dir.join(&path);
            }
        };

        for path in config.library_paths.values_mut() {
            to_absolute_path(path);
        }

        for path in config.include_paths.iter_mut() {
            to_absolute_path(path);
        }

        if let Some(path) = config.translation_path_bundle.as_mut() {
            to_absolute_path(path);
        }

        Self { config, project_file }
    }
}

/// Error returned by the `compile` function
#[derive(derive_more::Error, derive_more::Display, Debug)]
#[non_exhaustive]
pub enum CompileError {
    /// Cannot read environment variable CARGO_MANIFEST_DIR or OUT_DIR. The build script need to be run via cargo.
    #[display(
        "Cannot read environment variable CARGO_MANIFEST_DIR or OUT_DIR. The build script need to be run via cargo."
    )]
    NotRunViaCargo,
    /// Parse error. The error are printed in the stderr, and also are in the vector
    #[display("{_0:?}")]
    CompileError(#[error(not(source))] Vec<String>),
    /// Cannot write the generated file
    #[display("Cannot write the generated file: {_0}")]
    SaveError(std::io::Error),
}

fn project_file_path_in(manifest_dir: impl AsRef<Path>) -> PathBuf {
    manifest_dir.as_ref().join(PROJECT_FILE_NAME)
}

fn discover_project_file_in(manifest_dir: &Path) -> Result<Option<ProjectFile>, String> {
    let project_file_path = project_file_path_in(manifest_dir);
    println!("cargo:rerun-if-changed={}", project_file_path.display());

    match ProjectFile::load(&project_file_path) {
        Ok(project_file) => Ok(Some(project_file)),
        Err(error) => {
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io_error| io_error.kind() == std::io::ErrorKind::NotFound)
            {
                // TODO: Should this emit a warning?
                Ok(None)
            } else {
                Err(error.to_string())
            }
        }
    }
}

fn manifest_dir() -> Result<PathBuf, String> {
    #[cfg(test)]
    if let Some(manifest_dir_override) =
        TEST_MANIFEST_DIR_OVERRIDE.with(|override_| override_.borrow().clone())
    {
        return Ok(manifest_dir_override);
    }

    std::env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "Unable to determine CARGO_MANIFEST_DIR!".to_owned())
}

fn discover_project_file() -> Result<Option<ProjectFile>, String> {
    discover_project_file_in(&manifest_dir()?)
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
///
/// See also [`compile_with_config()`] if you want to specify a configuration.
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
    let manifest_path = std::path::PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").ok_or(CompileError::NotRunViaCargo)?,
    );
    let config = config.with_absolute_paths(&manifest_path);

    let path = manifest_path.join(relative_slint_file_path.as_ref());

    let absolute_rust_output_file_path =
        Path::new(&env::var_os("OUT_DIR").ok_or(CompileError::NotRunViaCargo)?).join(
            path.file_stem()
                .map(Path::new)
                .unwrap_or_else(|| Path::new("slint_out"))
                .with_extension("rs"),
        );

    #[cfg(feature = "experimental-module-builds")]
    if let Some(library_name) = config.config.library_name.clone() {
        println!("cargo::metadata=SLINT_LIBRARY_NAME={}", library_name);
        println!(
            "cargo::metadata=SLINT_LIBRARY_PACKAGE={}",
            std::env::var("CARGO_PKG_NAME").ok().unwrap_or_default()
        );
        println!("cargo::metadata=SLINT_LIBRARY_SOURCE={}", path.display());
        if let Some(rust_module) = &config.config.rust_module {
            println!("cargo::metadata=SLINT_LIBRARY_MODULE={}", rust_module);
        }
    }
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
    println!("cargo:rerun-if-env-changed=SLINT_LIVE_PREVIEW");

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

    if diag.has_errors()
        || (!diag.is_empty() && std::env::var("SLINT_COMPILER_DENY_WARNINGS").is_ok())
    {
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

    write!(code_formatter, "{generated}").map_err(CompileError::SaveError)?;
    dependencies.push(input_slint_file_path.as_ref().to_path_buf());

    let project_file_path = config
        .project_file
        .as_ref()
        .map(|project_file| project_file.source_path().to_owned())
        .unwrap_or_else(|| project_file_path_in(manifest_dir().unwrap()));
    dependencies.push(project_file_path);

    for er in doc.embedded_file_resources.borrow().iter() {
        if let Some(resource) = er.path.as_deref()
            && !resource.starts_with("builtin:")
        {
            dependencies.push(Path::new(resource).to_path_buf());
        }
    }

    code_formatter.sink.flush().map_err(CompileError::SaveError)?;

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
                println!("cargo:rustc-link-arg={option}");
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

#[cfg(test)]
thread_local! {
    static TEST_MANIFEST_DIR_OVERRIDE: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn root_path_prefix() -> std::path::PathBuf {
    #[cfg(windows)]
    return std::path::PathBuf::from("C:/");
    #[cfg(not(windows))]
    return std::path::PathBuf::from("/");
}

#[cfg(test)]
fn with_manifest_dir<R>(manifest_dir: &Path, f: impl FnOnce() -> R) -> R {
    static MANIFEST_DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = MANIFEST_DIR_LOCK.lock().unwrap();
    let previous_manifest_dir_override =
        TEST_MANIFEST_DIR_OVERRIDE.with(|override_| override_.borrow().clone());
    TEST_MANIFEST_DIR_OVERRIDE.with(|override_| {
        *override_.borrow_mut() = Some(manifest_dir.to_path_buf());
    });
    let result = f();
    TEST_MANIFEST_DIR_OVERRIDE.with(|override_| {
        *override_.borrow_mut() = previous_manifest_dir_override;
    });
    result
}

#[cfg(test)]
fn with_temp_test_dir<R>(test_name: &str, f: impl FnOnce(&Path) -> R) -> R {
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let test_dir =
        std::env::temp_dir().join("slint-build-tests").join(format!("{test_name}-{stamp}"));
    std::fs::create_dir_all(&test_dir).unwrap();
    let result = f(&test_dir);
    std::fs::remove_dir_all(test_dir).unwrap();
    result
}

#[test]
fn with_absolute_library_paths_test() {
    use std::path::PathBuf;

    let library_paths = std::collections::HashMap::from([
        ("relative".to_string(), PathBuf::from("some/relative/path")),
        ("absolute".to_string(), root_path_prefix().join("some/absolute/path")),
    ]);
    let config = CompilerConfiguration::new().with_library_paths(library_paths);

    let manifest_path = root_path_prefix().join("path/to/manifest");
    let absolute_config = config.clone().with_absolute_paths(&manifest_path);
    let relative = &absolute_config.config.library_paths["relative"];
    assert!(relative.is_absolute());
    assert!(relative.starts_with(&manifest_path));

    assert!(!absolute_config.config.library_paths["absolute"].starts_with(&manifest_path));
}

#[test]
fn with_absolute_include_paths_test() {
    use std::path::PathBuf;

    let config = CompilerConfiguration::new().with_include_paths(Vec::from([
        root_path_prefix().join("some/absolute/path"),
        PathBuf::from("some/relative/path"),
    ]));

    let manifest_path = root_path_prefix().join("path/to/manifest");
    let absolute_config = config.clone().with_absolute_paths(&manifest_path);
    assert_eq!(
        absolute_config.config.include_paths,
        Vec::from([
            root_path_prefix().join("some/absolute/path"),
            manifest_path.join("some/relative/path"),
        ])
    )
}

#[test]
fn project_file_defaults_use_project_file_directory_for_relative_paths() {
    use std::{collections::HashMap, fs};

    with_temp_test_dir("relative-paths", |test_root| {
        let project_file = project_file_path_in(test_root);
        fs::write(
            &project_file,
            r#"{
                "include-directories": ["project-include"],
                "library-paths": {"widgets": "project-lib.slint"},
                "style": "project-style"
            }"#,
        )
        .unwrap();

        let test_root = fs::canonicalize(test_root).unwrap();
        let merged = with_manifest_dir(&test_root, CompilerConfiguration::new)
            .with_absolute_paths(&test_root);

        assert_eq!(merged.config.include_paths, vec![test_root.join("project-include")]);
        assert_eq!(
            merged.config.library_paths,
            HashMap::from([("widgets".to_string(), test_root.join("project-lib.slint"))])
        );
        assert_eq!(merged.config.style.as_deref(), Some("project-style"));
        let project_file = fs::canonicalize(&project_file).unwrap();
        assert_eq!(
            merged.project_file.as_ref().map(ProjectFile::source_path),
            Some(project_file.as_path())
        );
    });
}

#[test]
fn project_file_defaults_are_overridden_by_explicit_settings() {
    use std::{collections::HashMap, fs, path::PathBuf};

    with_temp_test_dir("override-precedence", |test_root| {
        let project_file = project_file_path_in(test_root);
        fs::write(
            &project_file,
            r#"{
                "include-directories": ["project-include"],
                "library-paths": {"widgets": "project-lib.slint"},
                "style": "project-style"
            }"#,
        )
        .unwrap();

        let test_root = fs::canonicalize(test_root).unwrap();
        let merged = with_manifest_dir(&test_root, || {
            CompilerConfiguration::new()
                .with_include_paths(vec![PathBuf::from("build-include")])
                .with_library_paths(HashMap::from([(
                    "widgets".to_string(),
                    PathBuf::from("build-lib.slint"),
                )]))
                .with_style("build-style".into())
        })
        .with_absolute_paths(&test_root);

        assert_eq!(merged.config.include_paths, vec![test_root.join("build-include")]);
        assert_eq!(
            merged.config.library_paths,
            HashMap::from([("widgets".to_string(), test_root.join("build-lib.slint"))])
        );
        assert_eq!(merged.config.style.as_deref(), Some("build-style"));
    });
}

#[test]
fn compiler_configuration_defaults_without_project_file() {
    use std::fs;

    with_temp_test_dir("optional-project-file", |test_root| {
        let untouched = with_manifest_dir(test_root, CompilerConfiguration::new);
        assert!(untouched.config.style.is_none());
        assert!(untouched.project_file.is_none());

        let project_file = project_file_path_in(test_root);
        fs::write(&project_file, r#"{"style":"project-style"}"#).unwrap();

        let merged =
            with_manifest_dir(test_root, CompilerConfiguration::new).with_absolute_paths(test_root);
        assert_eq!(merged.config.style.as_deref(), Some("project-style"));
    });
}

#[test]
fn project_file_discovery_is_exactly_manifest_dir_relative() {
    use std::fs;

    with_temp_test_dir("discovery-exact-manifest", |test_root| {
        let parent_dir = test_root.join("parent");
        let manifest_dir = parent_dir.join("manifest");
        let parent_project_file = parent_dir.join(PROJECT_FILE_NAME);
        let manifest_project_file = project_file_path_in(&manifest_dir);

        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(&parent_project_file, r#"{"style":"parent"}"#).unwrap();

        assert!(discover_project_file_in(&manifest_dir).unwrap().is_none());

        fs::write(&manifest_project_file, r#"{"style":"fluent"}"#).unwrap();
        let discovered = discover_project_file_in(&manifest_dir).unwrap().unwrap();
        assert_eq!(discovered.source_path(), fs::canonicalize(&manifest_project_file).unwrap());
    });
}

#[test]
fn project_file_discovery_loads_present_paths() {
    use std::fs;

    with_temp_test_dir("discovery-present-paths", |test_root| {
        let project_file = project_file_path_in(test_root);

        fs::create_dir_all(&project_file).unwrap();

        let error = discover_project_file_in(test_root).unwrap_err();
        assert!(error.to_string().contains("Is a directory"));
    });
}

#[test]
fn output_path_compilation_tracks_project_file_dependency() {
    use std::fs;

    with_temp_test_dir("output-deps", |test_root| {
        let project_file = project_file_path_in(test_root);
        let input_file = test_root.join("main.slint");
        let output_file = test_root.join("main.rs");

        fs::write(&project_file, r#"{"style":"fluent"}"#).unwrap();
        fs::write(&input_file, "export component Test inherits Rectangle {}").unwrap();

        let config = with_manifest_dir(test_root, CompilerConfiguration::new);
        let dependencies = compile_with_output_path(&input_file, &output_file, config).unwrap();

        let project_file = i_slint_compiler::pathutils::clean_path(&project_file);
        assert!(
            dependencies.iter().any(|dependency| dependency == &project_file),
            "expected: {:?}\ndeps: {dependencies:#?}",
            project_file
        );
    });
}

#[test]
fn output_path_compilation_tracks_missing_project_file_dependency() {
    use std::fs;

    with_temp_test_dir("missing-project-deps", |test_root| {
        let project_file = project_file_path_in(test_root);
        let input_file = test_root.join("main.slint");
        let output_file = test_root.join("main.rs");

        fs::write(&input_file, "export component Test inherits Rectangle {}").unwrap();

        let dependencies = with_manifest_dir(test_root, || {
            let config = CompilerConfiguration::new();
            compile_with_output_path(&input_file, &output_file, config).unwrap()
        });

        let project_file = i_slint_compiler::pathutils::clean_path(&project_file);
        assert!(
            dependencies
                .iter()
                .any(|dependency| i_slint_compiler::pathutils::clean_path(dependency)
                    == project_file),
            "expected: {:?}\ndeps: {dependencies:#?}",
            project_file
        );
    });
}
