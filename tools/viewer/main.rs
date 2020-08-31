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

    let mut include_paths = args.include_paths;
    if args.style == "qt" || args.style == "native" {
        // FIXME: that's not how it should work
        include_paths.push(
            [env!("CARGO_MANIFEST_DIR"), "..", "..", "sixtyfps_runtime", "rendering_backend", "qt"]
                .iter()
                .collect(),
        );
    }

    let c = match sixtyfps_interpreter::load(source, &args.path, &include_paths) {
        Ok(c) => c,
        Err(diag) => {
            diag.print();
            std::process::exit(-1);
        }
    };

    let component = c.create();
    component.window().run(component.borrow(), component.root_item());
    Ok(())
}
