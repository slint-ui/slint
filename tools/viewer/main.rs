/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    #[structopt(short = "I", name = "include path for other .60 files", number_of_values = 1)]
    include_paths: Vec<std::path::PathBuf>,

    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,

    /// The style name (empty, or 'qt')
    #[structopt(long, name = "style name", default_value)]
    style: String,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();
    let source = std::fs::read_to_string(&args.path)?;

    let mut compiler_config = sixtyfps_compilerlib::CompilerConfiguration::new(
        sixtyfps_compilerlib::generator::OutputFormat::Interpreter,
    );
    compiler_config.include_paths = args.include_paths;
    compiler_config.style = if args.style.is_empty() { None } else { Some(args.style) };

    let c = match spin_on::spin_on(sixtyfps_interpreter::load(source, args.path, compiler_config)) {
        (Ok(c), warnings) => {
            warnings.print();
            c
        }
        (Err(()), errors) => {
            errors.print();
            std::process::exit(-1);
        }
    };

    let component = c.create();
    component.window().show();
    sixtyfps_interpreter::run_event_loop();
    component.window().hide();
    Ok(())
}
