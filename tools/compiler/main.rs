// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use clap::{Parser, ValueEnum};
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::*;
use std::io::Write;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Embedding {
    /// Embed resources using absolute paths on the build system (alias: false)
    #[value(alias = "false", name = "as-paths")]
    AsAbsolutePath,
    /// Embed contents of resource files (alias: true)
    #[value(alias = "true", name = "as-contents")]
    AsFiles,
    /// Embed in a format optimized for the software renderer. This
    /// option falls back to as-contents when the software-renderer is not
    /// used
    #[value(name = "as-textures")]
    SoftwareRenderer,
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

    /// Path to .slint file ('-' for stdin)
    #[arg(name = "file", action)]
    path: std::path::PathBuf,

    /// The style name ('native' or 'fluent')
    #[arg(long, name = "style name", action)]
    style: Option<String>,

    /// Generate a dependency file
    #[arg(name = "dependency file", long = "depfile", number_of_values = 1, action)]
    depfile: Option<std::path::PathBuf>,

    /// Declare which resources to embed
    #[arg(long, name = "value", value_enum)]
    embed_resources: Option<Embedding>,

    /// Sets the output file ('-' for stdout)
    #[arg(name = "file to generate", short = 'o', default_value = "-", action)]
    output: std::path::PathBuf,
}

fn main() -> std::io::Result<()> {
    proc_macro2::fallback::force(); // avoid a abort if panic=abort is set
    let args = Cli::parse();
    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse_file(&args.path, &mut diag);
    //println!("{:#?}", syntax_node);
    if diag.has_error() {
        diag.print();
        std::process::exit(-1);
    }
    let mut compiler_config = CompilerConfiguration::new(args.format);

    // Override defaults from command line:
    if let Some(embed) = args.embed_resources {
        compiler_config.embed_resources = match embed {
            Embedding::AsAbsolutePath => EmbedResourcesKind::OnlyBuiltinResources,
            Embedding::AsFiles => EmbedResourcesKind::EmbedAllResources,
            #[cfg(feature = "software-renderer")]
            Embedding::SoftwareRenderer => EmbedResourcesKind::EmbedTextures,
            #[cfg(not(feature = "software-renderer"))]
            Embedding::SoftwareRenderer => EmbedResourcesKind::EmbedAllResources,
        };
    }

    compiler_config.include_paths = args.include_paths;
    if let Some(style) = args.style {
        compiler_config.style = Some(style);
    }
    let syntax_node = syntax_node.expect("diags contained no compilation errors");
    let (doc, diag) = spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));

    let diag = diag.check_and_exit_on_error();

    if args.output == std::path::Path::new("-") {
        generator::generate(args.format, &mut std::io::stdout(), &doc)?;
    } else {
        generator::generate(args.format, &mut std::fs::File::create(&args.output)?, &doc)?;
    }

    if let Some(depfile) = args.depfile {
        let mut f = std::fs::File::create(depfile)?;
        write!(f, "{}:", args.output.display())?;
        for x in &diag.all_loaded_files {
            if x.is_absolute() {
                write!(f, " {}", x.display())?;
            }
        }
        for resource in doc.root_component.embedded_file_resources.borrow().keys() {
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
