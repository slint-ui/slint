// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Reports the coverage of `.slint` files compiled for Slint SC.
//!
//! `slint-compiler --slint-sc --coverage` instruments the generated code with
//! one empty marker function per coverage point of the `.slint` source and
//! writes a `.slintcov` map locating the points. Tests built with
//! `-C instrument-coverage` (`cargo llvm-cov`) count the markers like any
//! other function; this tool reads the counts from the profile and writes the
//! coverage of the `.slint` files in the lcov format.

use clap::Parser;
use slint_sc_coverage::markers;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// A `.slintcov` map written by `slint-compiler --slint-sc --coverage`, or a directory to
    /// search for them.
    #[arg(long = "map", value_name = "PATH", required = true, num_args = 1..)]
    maps: Vec<PathBuf>,

    /// An LLVM profile of a run of the instrumented code: `.profraw`, `.profdata`, or text. Merged
    /// with `llvm-profdata`, found through `LLVM_PROFDATA`, the Rust toolchain's llvm-tools, or PATH.
    #[arg(long = "profile", value_name = "PATH", required = true, num_args = 1..)]
    profiles: Vec<PathBuf>,

    /// Report the `.slint` paths relative to this directory.
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
    let base_dir = match &args.base_dir {
        Some(dir) => dir.clone(),
        None => std::env::current_dir()?,
    };
    let mut map_files = Vec::new();
    for path in &args.maps {
        collect_maps(path, &mut map_files)?;
    }
    if map_files.is_empty() {
        return Err("no .slintcov map found".into());
    }

    let counts = markers::parse_profile(&markers::merge_profiles(&args.profiles)?);

    let mut report = slint_sc_coverage::Report::default();
    let mut seen_hashes = std::collections::BTreeSet::new();
    let mut skipped = 0;
    for path in &map_files {
        let text = std::fs::read_to_string(path)?;
        let map = markers::parse_map(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if !seen_hashes.insert(map.hash.clone()) {
            continue;
        }
        if !markers::add(&map, &counts, &mut report) {
            skipped += 1;
        }
    }
    if skipped > 0 {
        eprintln!(
            "{skipped} of {} maps skipped: not in the profile, as their code was never built with \
             coverage or they are left over from an earlier build",
            seen_hashes.len()
        );
    }
    if report.is_empty() {
        return Err("none of the maps matches the profile".into());
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

/// The `.slintcov` files at `path`: itself, or the ones below the directory.
fn collect_maps(path: &Path, maps: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_dir() {
        collect_maps_below(path, maps)
    } else if path.is_file() {
        maps.push(path.to_path_buf());
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{}: not found", path.display()),
        ))
    }
}

/// The directory tree may be a whole `target/`, so decide from the entries
/// alone without stat'ing each.
fn collect_maps_below(dir: &Path, maps: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_maps_below(&path, maps)?;
        } else if path.extension().is_some_and(|ext| ext == "slintcov") {
            maps.push(path);
        }
    }
    Ok(())
}
