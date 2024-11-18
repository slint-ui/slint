// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use clap::{Parser, ValueEnum};
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::*;
use itertools::Itertools;
use std::io::{BufWriter, Write};

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
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Set output format
    #[arg(short = 'f', long = "format", default_value = "cpp", action)]
    format: generator::OutputFormat,

    /// Include path for other .slint files
    #[arg(short = 'I', name = "include path", number_of_values = 1, action)]
    include_paths: Vec<std::path::PathBuf>,

    /// The argument should be in the format `<library>=<path>` specifying the
    /// name of the library and the path to the library directory or a .slint
    /// entry-point file.
    #[arg(short = 'L', name = "library path", number_of_values = 1, action)]
    library_paths: Vec<String>,

    /// Path to .slint file ('-' for stdin)
    #[arg(name = "file", action)]
    path: std::path::PathBuf,

    /// The style name ('native' or 'fluent')
    #[arg(long, name = "style name", action)]
    style: Option<String>,

    /// The constant scale factor to apply for embedded assets and set by default on the window.
    #[arg(long, name = "scale factor", action)]
    scale_factor: Option<f64>,

    /// Generate a dependency file
    #[arg(name = "dependency file", long = "depfile", number_of_values = 1, action)]
    depfile: Option<std::path::PathBuf>,

    /// Declare which resources to embed
    #[arg(long, name = "value", value_enum)]
    embed_resources: Option<Embedding>,

    /// Sets the output file ('-' for stdout)
    #[arg(name = "file to generate", short = 'o', default_value = "-", action)]
    output: std::path::PathBuf,

    /// Translation domain
    #[arg(long = "translation-domain", action)]
    translation_domain: Option<String>,

    /// Bundle translations from the specified path.
    /// Translations files should be in the gettext .po format and should be found in
    /// `<path>/<lang>/LC_MESSAGES/<domain>.po`
    #[arg(long = "bundle-translations", name = "path", action)]
    bundle_translations: Option<std::path::PathBuf>,

    /// C++ namespace
    #[arg(long = "cpp-namespace", name = "C++ namespace")]
    cpp_namespace: Option<String>,

    /// C++ files to generate (0 for header-only output)
    #[arg(long = "cpp-file", name = "C++ file to generate", number_of_values = 1, action)]
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
                eprintln!("C++ files option was set but the output format is not C++ - ignorning");
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
                .map_or(false, |f| f.is_builtin())
            {
                write!(f, " {}", resource)?;
            }
        }

        writeln!(f)?;
    }
    diag.print_warnings_and_exit_on_error();
    Ok(())
}
