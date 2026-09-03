// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Reports the coverage of `.slint` files compiled for Slint SC.
//!
//! The Slint SC compiler marks the coverage points of the `.slint` source in
//! the generated code with `origin!("...")`, a macro that expands to nothing
//! and names the point's location. This tool finds
//! the marks in the generated code and, from LLVM's coverage of that code,
//! takes the execution count of the region each mark precedes as the point's
//! hit count, and writes the coverage of the `.slint` files in the lcov
//! format.

use clap::Parser;
use slint_sc_coverage::marks;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Generated code, or a directory holding it. Only the files the coverage export names are
    /// read.
    #[arg(long = "generated", value_name = "PATH", required = true, num_args = 1..)]
    generated: Vec<PathBuf>,

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

    let mut exports: Vec<String> = Vec::new();
    for path in &args.exports {
        exports.push(std::fs::read_to_string(path)?);
    }
    if !args.objects.is_empty() {
        exports.push(marks::export_coverage(&args.objects, &args.profiles)?);
    }
    let mut regions = marks::Regions::default();
    for export in &exports {
        regions.parse(export, &base_dir)?;
    }

    let generated: Vec<PathBuf> =
        args.generated.iter().map(|path| path.canonicalize()).collect::<Result<_, _>>()?;
    let mut report = slint_sc_coverage::Report::default();
    let mut marked = 0;
    for (path, file_regions) in &regions.files {
        if !generated.iter().any(|dir| path.starts_with(dir)) {
            continue;
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        marked += marks::add(&text, file_regions, &mut report)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    if marked == 0 {
        return Err("no coverage mark found in the generated code the export names".into());
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
