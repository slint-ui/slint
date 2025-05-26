// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use clap::Args;

pub mod fmt;
#[cfg(not(target_arch = "wasm32"))]
pub mod tool;
pub mod writer;

#[derive(Args, Clone)]
pub struct Format {
    #[arg(name = "path to .slint file(s)", action)]
    pub paths: Vec<std::path::PathBuf>,

    /// modify the file inline instead of printing to stdout
    #[arg(short, long, action)]
    pub inline: bool,
}

pub fn run_formatter(args: Format) -> ! {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = tool::run(&args.paths, args.inline).map_err(|e| {
            eprintln!("{e}");
            std::process::exit(1);
        });
    }
    std::process::exit(0);
}
