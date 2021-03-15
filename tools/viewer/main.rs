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

    /// An optionally registered font
    #[structopt(long, name = "load font")]
    load_font: Option<Vec<String>>,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();

    args.load_font.map(|fonts| {
        fonts.iter().for_each(|font_path| {
            if let Err(app_font_err) = sixtyfps_interpreter::register_font_from_path(&font_path) {
                eprintln!("Error loading app font {}: {}", font_path, app_font_err);
            }
        });
    });

    let mut compiler_config =
        sixtyfps_interpreter::CompilerConfiguration::new().with_include_paths(args.include_paths);
    if !args.style.is_empty() {
        compiler_config = compiler_config.with_style(args.style);
    }

    let (c, diags) = spin_on::spin_on(sixtyfps_interpreter::ComponentDefinition::from_path(
        args.path,
        compiler_config,
    ));
    sixtyfps_interpreter::print_diagnostics(&diags);

    let c = match c {
        Some(c) => c,
        None => std::process::exit(-1),
    };

    let component = c.create();
    component.run();
    Ok(())
}
