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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum ObjectBinaryFormat {
    Coff,
    Elf,
    MachO,
    Pe,
    Wasm,
    Xcoff,
}

impl From<ObjectBinaryFormat> for object::BinaryFormat {
    fn from(value: ObjectBinaryFormat) -> Self {
        match value {
            ObjectBinaryFormat::Coff => object::BinaryFormat::Coff,
            ObjectBinaryFormat::Elf => object::BinaryFormat::Elf,
            ObjectBinaryFormat::MachO => object::BinaryFormat::MachO,
            ObjectBinaryFormat::Pe => object::BinaryFormat::Pe,
            ObjectBinaryFormat::Wasm => object::BinaryFormat::Wasm,
            ObjectBinaryFormat::Xcoff => object::BinaryFormat::Xcoff,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum ObjectEndianness {
    #[allow(non_camel_case_types)]
    Little_Endian,
    #[allow(non_camel_case_types)]
    Big_Endian,
}

impl From<ObjectEndianness> for object::Endianness {
    fn from(value: ObjectEndianness) -> Self {
        match value {
            ObjectEndianness::Little_Endian => object::Endianness::Little,
            ObjectEndianness::Big_Endian => object::Endianness::Big,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum ObjectArchitecture {
    Unknown,
    Aarch64,
    #[allow(non_camel_case_types)]
    Aarch64_Ilp32,
    Arm,
    Avr,
    Bpf,
    Csky,
    I386,
    X86_64,
    #[allow(non_camel_case_types)]
    X86_64_X32,
    Hexagon,
    LoongArch64,
    Mips,
    Mips64,
    Msp430,
    PowerPc,
    PowerPc64,
    Riscv32,
    Riscv64,
    S390x,
    Sbf,
    Sharc,
    Sparc,
    Sparc32Plus,
    Sparc64,
    Wasm32,
    Wasm64,
    Xtensa,
}

impl From<ObjectArchitecture> for object::Architecture {
    fn from(value: ObjectArchitecture) -> Self {
        match value {
            ObjectArchitecture::Unknown => object::Architecture::Unknown,
            ObjectArchitecture::Aarch64 => object::Architecture::Aarch64,
            ObjectArchitecture::Aarch64_Ilp32 => object::Architecture::Aarch64_Ilp32,
            ObjectArchitecture::Arm => object::Architecture::Arm,
            ObjectArchitecture::Avr => object::Architecture::Avr,
            ObjectArchitecture::Bpf => object::Architecture::Bpf,
            ObjectArchitecture::Csky => object::Architecture::Csky,
            ObjectArchitecture::I386 => object::Architecture::I386,
            ObjectArchitecture::X86_64 => object::Architecture::X86_64,
            ObjectArchitecture::X86_64_X32 => object::Architecture::X86_64_X32,
            ObjectArchitecture::Hexagon => object::Architecture::Hexagon,
            ObjectArchitecture::LoongArch64 => object::Architecture::LoongArch64,
            ObjectArchitecture::Mips => object::Architecture::Mips,
            ObjectArchitecture::Mips64 => object::Architecture::Mips64,
            ObjectArchitecture::Msp430 => object::Architecture::Msp430,
            ObjectArchitecture::PowerPc => object::Architecture::PowerPc,
            ObjectArchitecture::PowerPc64 => object::Architecture::PowerPc64,
            ObjectArchitecture::Riscv32 => object::Architecture::Riscv32,
            ObjectArchitecture::Riscv64 => object::Architecture::Riscv64,
            ObjectArchitecture::S390x => object::Architecture::S390x,
            ObjectArchitecture::Sbf => object::Architecture::Sbf,
            ObjectArchitecture::Sharc => object::Architecture::Sharc,
            ObjectArchitecture::Sparc => object::Architecture::Sparc,
            ObjectArchitecture::Sparc32Plus => object::Architecture::Sparc32Plus,
            ObjectArchitecture::Sparc64 => object::Architecture::Sparc64,
            ObjectArchitecture::Wasm32 => object::Architecture::Wasm32,
            ObjectArchitecture::Wasm64 => object::Architecture::Wasm64,
            ObjectArchitecture::Xtensa => object::Architecture::Xtensa,
        }
    }
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

    #[arg(name = "resources object file (only for C++)", long = "resource-object-file")]
    resources_object_file: Option<std::path::PathBuf>,

    #[arg(name = "resource object binary format (only for C++)", long = "resource-object-format")]
    resources_object_binary_format: Option<ObjectBinaryFormat>,

    #[arg(name = "resource object endianess (only for C++)", long = "resource-object-endianess")]
    resources_object_endianess: Option<ObjectEndianness>,

    #[arg(
        name = "resource object architecture (only for C++)",
        long = "resource-object-architecture"
    )]
    resources_object_architecture: Option<ObjectArchitecture>,

    /// Translation domain
    #[arg(long = "translation-domain", action)]
    translation_domain: Option<String>,

    /// C++ namespace
    #[arg(long = "cpp-namespace", name = "C++ namespace")]
    cpp_namespace: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        match &mut format {
            generator::OutputFormat::Cpp(ref mut config) => {
                config.namespace = args.cpp_namespace;
            }
            _ => eprintln!("C++ namespace option was set. Output format will be C++."),
        }
    }

    if args.resources_object_file.is_some() {
        match &mut format {
            generator::OutputFormat::Cpp(ref mut config) => {
                let Some(binary_format) = args.resources_object_binary_format else {
                    eprintln!("resources-object-file requires resource-object-format argument");
                    std::process::exit(-1);
                };
                let Some(endianess) = args.resources_object_endianess else {
                    eprintln!("resources-object-file requires resource-object-endianess argument");
                    std::process::exit(-1);
                };
                let Some(architecture) = args.resources_object_architecture else {
                    eprintln!(
                        "resources-object-file requires resource-object-architecture argument"
                    );
                    std::process::exit(-1);
                };
                config.object_file_resource_config =
                    Some(i_slint_compiler::generator::cpp::ObjectFileResourceConfig {
                        path: args.resources_object_file.unwrap(),
                        binary_format: binary_format.into(),
                        endianess: endianess.into(),
                        architecture: architecture.into(),
                    });
            }
            _ => eprintln!("C++ namespace option was set. Output format will be C++."),
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
