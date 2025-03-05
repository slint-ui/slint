// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use clap::{Parser, ValueEnum};
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::*;
use itertools::Itertools;
use std::io::{BufWriter, Write};

#[cfg(all(feature = "jemalloc", not(target_os = "windows")))]
use tikv_jemallocator::Jemalloc;

#[cfg(all(feature = "jemalloc", not(target_os = "windows")))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Embedding {
    /// Embed resources using absolute paths on the build system (alias: false)
    #[value(alias = "false")]
    AsAbsolutePath,
    /// Embed contents of resource files (alias: true)
    #[value(alias = "true")]
    EmbedFiles,
    /// Embed in a format optimized for the software renderer. This
    /// option falls back to `embed-files` if the software-renderer is not
    /// used
    #[cfg(feature = "software-renderer")]
    EmbedForSoftwareRenderer,
    /// Same as "embed-files-for-software-renderer" but use Signed Distance Field (SDF) to render fonts.
    /// This produces smaller binaries, but may result in slightly inferior visual output and slower rendering.
    #[cfg(all(feature = "software-renderer", feature = "sdf-fonts"))]
    EmbedForSoftwareRendererWithSdf,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Set the output format for generated code.
    /// Possible values: 'cpp' for C++ code or 'rust' for Rust code.
    #[arg(short = 'f', long = "format", default_value = "cpp")]
    format: generator::OutputFormat,

    /// Specify include paths for imported .slint files or image resources.
    /// This is used for including external .slint files or image resources referenced by '@image-url'.
    #[arg(short = 'I', name = "include path", number_of_values = 1)]
    include_paths: Vec<std::path::PathBuf>,

    /// Define library paths in the format `<library>=<path>`.
    /// This can point to either a library directory or a .slint entry-point file.
    #[arg(short = 'L', name = "library path", number_of_values = 1)]
    library_paths: Vec<String>,

    /// Specify the path to the main .slint file to compile.
    /// Use '-' to read from stdin.
    #[arg(name = "file")]
    path: std::path::PathBuf,

    /// Set the style for the UI (e.g., 'native' or 'fluent').
    #[arg(long, name = "style name")]
    style: Option<String>,

    /// Apply a constant scale factor to embedded assets, typically for high-DPI displays.
    /// This scale factor is also applied to the window by default.
    #[arg(long, name = "scale factor")]
    scale_factor: Option<f64>,

    /// Generate a dependency file for build systems like CMake or Ninja.
    /// This file is similar to the output of `gcc -M`.
    #[arg(long = "depfile", name = "dependency file", number_of_values = 1)]
    depfile: Option<std::path::PathBuf>,

    /// Declare which resources to embed into the final output.
    #[arg(long, name = "value", value_enum)]
    embed_resources: Option<Embedding>,

    /// Set the output file for the generated code.
    /// Use '-' to output to stdout.
    #[arg(short = 'o', name = "output file", default_value = "-")]
    output: std::path::PathBuf,

    /// Set the translation domain for translatable strings.
    /// This is used to manage translation of strings in the UI.
    #[arg(long = "translation-domain")]
    translation_domain: Option<String>,

    /// Bundle translations from a specified path.
    /// Translation files should be in the gettext `.po` format and follow the directory structure:
    /// `<path>/<lang>/LC_MESSAGES/<domain>.po`.
    #[arg(long = "bundle-translations", name = "path")]
    bundle_translations: Option<std::path::PathBuf>,

    /// Define the C++ namespace for generated code.
    #[arg(long = "cpp-namespace", name = "C++ namespace")]
    cpp_namespace: Option<String>,

    /// Specify C++ files to generate.
    ///
    /// The header file (.h) is always generated in the file specified by `-o`.
    /// If `--cpp-file` is not set, all code will be generated in the header file.
    /// If set, function definitions are placed in the specified `.cpp` file.
    /// If specified multiple times, the definitions are split across multiple `.cpp` files.
    #[arg(long = "cpp-file", name = "output .cpp file", number_of_values = 1)]
    cpp_files: Vec<std::path::PathBuf>,
}

fn main() -> std::io::Result<()> {
    proc_macro2::fallback::force(); // avoid a abort if panic=abort is set
    let args = Cli::parse();
    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse_file(&args.path, &mut diag);
    //println!("{:#?}", syntax_node);
    if diag.has_errors() {
        diag.print();
        std::process::exit(-1);
    }

    let mut format = args.format.clone();

    if args.cpp_namespace.is_some() {
        if !matches!(format, generator::OutputFormat::Cpp(..)) {
            eprintln!("C++ namespace option was set. Output format will be C++.");
        }
        format = generator::OutputFormat::Cpp(generator::cpp::Config {
            namespace: args.cpp_namespace,
            ..Default::default()
        });
    }

    if !args.cpp_files.is_empty() {
        match &mut format {
            generator::OutputFormat::Cpp(ref mut config) => {
                config.cpp_files = args.cpp_files;

                if args.output == std::path::Path::new("-") {
                    eprintln!("--cpp-file can only be used together with -o");
                    std::process::exit(1);
                }

                config.header_include = args.output.to_string_lossy().to_string();
            }
            _ => {
                eprintln!("C++ files option was set but the output format is not C++ - ignoring");
            }
        }
    }

    let mut compiler_config = CompilerConfiguration::new(format.clone());
    compiler_config.translation_domain = args.translation_domain;

    // Override defaults from command line:
    if let Some(embed) = args.embed_resources {
        compiler_config.embed_resources = match embed {
            Embedding::AsAbsolutePath => EmbedResourcesKind::OnlyBuiltinResources,
            Embedding::EmbedFiles => EmbedResourcesKind::EmbedAllResources,
            #[cfg(feature = "software-renderer")]
            Embedding::EmbedForSoftwareRenderer => EmbedResourcesKind::EmbedTextures,
            #[cfg(all(feature = "software-renderer", feature = "sdf-fonts"))]
            Embedding::EmbedForSoftwareRendererWithSdf => {
                compiler_config.use_sdf_fonts = true;
                EmbedResourcesKind::EmbedTextures
            }
        };
    }

    compiler_config.include_paths = args.include_paths;
    compiler_config.library_paths = args
        .library_paths
        .iter()
        .filter_map(|entry| entry.split('=').collect_tuple().map(|(k, v)| (k.into(), v.into())))
        .collect();
    if let Some(style) = args.style {
        compiler_config.style = Some(style);
    }
    if let Some(constant_scale_factor) = args.scale_factor {
        compiler_config.const_scale_factor = constant_scale_factor;
    }
    if let Some(path) = args.bundle_translations {
        compiler_config.translation_path_bundle = Some(path);
    }
    let syntax_node = syntax_node.expect("diags contained no compilation errors");
    let (doc, diag, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));

    let diag = diag.check_and_exit_on_error();

    if args.output == std::path::Path::new("-") {
        generator::generate(format, &mut std::io::stdout(), &doc, &loader.compiler_config)?;
    } else {
        generator::generate(
            format,
            &mut BufWriter::new(std::fs::File::create(&args.output)?),
            &doc,
            &loader.compiler_config,
        )?;
    }

    if let Some(depfile) = args.depfile {
        let mut f = BufWriter::new(std::fs::File::create(depfile)?);
        write!(f, "{}: {}", args.output.display(), args.path.display())?;
        for x in &diag.all_loaded_files {
            if x.is_absolute() {
                write!(f, " {}", x.display())?;
            }
        }
        for resource in doc.embedded_file_resources.borrow().keys() {
            if !fileaccess::load_file(std::path::Path::new(resource))
                .is_some_and(|f| f.is_builtin())
            {
                write!(f, " {resource}")?;
            }
        }

        writeln!(f)?;
    }
    diag.print_warnings_and_exit_on_error();
    Ok(())
}
