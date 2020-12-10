/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use sixtyfps_compilerlib::*;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    /// Set output format
    #[structopt(short = "f", long = "format", default_value = "cpp")]
    format: generator::OutputFormat,

    #[structopt(short = "I", name = "include path for other .60 files", number_of_values = 1)]
    include_paths: Vec<std::path::PathBuf>,

    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();
    let (syntax_node, diag) = parser::parse_file(&args.path)?;
    //println!("{:#?}", syntax_node);
    if diag.has_error() {
        diag.print();
        std::process::exit(-1);
    }
    let mut compiler_config = CompilerConfiguration::new(args.format);
    compiler_config.include_paths = args.include_paths;
    let (doc, diag) = spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));

    let mut diag = diag.check_and_exit_on_error();

    generator::generate(args.format, &mut std::io::stdout(), &doc, &mut diag)?;
    diag.print_warnings_and_exit_on_error();
    Ok(())
}
