// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Reports the coverage of `.slint` files compiled for Slint SC.
//!
//! The Slint SC compiler writes, next to the generated code, a map of its
//! coverage points of the `.slint` source and of the ranges of the code
//! that are one. This tool takes, from LLVM's coverage of the generated
//! code, the execution count of the code at each range as the point's hit
//! count, and writes the coverage of the `.slint` files in the lcov format.

use clap::Parser;
use slint_sc_coverage::source_map;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// An `llvm-cov export` in JSON (`cargo llvm-cov report --json`) of the instrumented code.
    #[arg(long = "export", value_name = "FILE", num_args = 1..)]
    exports: Vec<PathBuf>,

    /// A binary of the instrumented code, whose coverage is exported from `--profile` with
    /// `llvm-cov`, for binaries cargo-llvm-cov does not know of.
    #[arg(long = "object", value_name = "BINARY", num_args = 1.., requires = "profiles")]
    objects: Vec<PathBuf>,

    /// An LLVM profile, `.profraw` or `.profdata`, of a run of the `--object` binaries.
    #[arg(long = "profile", value_name = "PATH", num_args = 1..)]
    profiles: Vec<PathBuf>,

    /// The directory the export's relative paths are relative to, and to report the `.slint`
    /// paths relative to.
    #[arg(long = "base-dir", value_name = "DIR")]
    base_dir: Option<PathBuf>,

    /// Exit with an error when a coverage point was never reached.
    #[arg(long, action)]
    fail_on_gaps: bool,

    /// The lcov file to write, standard output when omitted.
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    if args.exports.is_empty() && args.objects.is_empty() {
        return Err("no coverage given: use --export, or --object with --profile".into());
    }
    let base_dir = match &args.base_dir {
        Some(dir) => dir.clone(),
        None => std::env::current_dir()?,
    };

    let mut regions = source_map::Regions::default();
    for path in &args.exports {
        regions.parse(&std::fs::read_to_string(path)?, &base_dir)?;
    }
    if !args.objects.is_empty() {
        regions.parse(&source_map::export_coverage(&args.objects, &args.profiles)?, &base_dir)?;
    }

    // The generated code among the files the export names is the one with a
    // map beside it.
    let mut report = slint_sc_coverage::Report::default();
    let mut mapped = 0;
    for (path, file_regions) in &regions.files {
        let map = path.with_extension("slintcov");
        let Ok(text) = std::fs::read_to_string(&map) else { continue };
        mapped += source_map::add(&text, file_regions, &mut report)
            .map_err(|e| format!("{}: {e}", map.display()))?;
    }
    if mapped == 0 {
        return Err("no coverage map found beside the generated code the export names".into());
    }

    let lcov = report.lcov(&base_dir);
    match &args.output {
        Some(path) => std::fs::write(path, lcov)?,
        None => print!("{lcov}"),
    }
    let gaps = report.summary(&base_dir);
    if args.fail_on_gaps && gaps > 0 {
        return Err(format!("{gaps} coverage points were never reached").into());
    }
    Ok(())
}
