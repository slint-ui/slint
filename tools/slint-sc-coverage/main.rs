// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Reports the coverage of `.slint` files compiled for Slint SC.
//!
//! `slint-compiler --slint-sc --coverage` instruments the generated code to
//! count its coverage points. A test writes the counters' profile, which
//! names the points' locations, and this tool joins the profiles into the
//! coverage of the `.slint` files in the lcov format.

use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// A profile written from the `SLINT_SC_COVERAGE` static of instrumented code, or a directory
    /// to search for `.slintcov` files.
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
    let mut profile_files = Vec::new();
    for path in &args.profiles {
        collect_profiles(path, &mut profile_files)?;
    }
    if profile_files.is_empty() {
        return Err("no .slintcov profile found".into());
    }

    let mut profiles = Vec::new();
    for path in &profile_files {
        let text = std::fs::read_to_string(path)?;
        let profile = slint_sc_coverage::profile::parse(&text)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        profiles.push(profile);
    }
    let mut report = slint_sc_coverage::Report::default();
    slint_sc_coverage::profile::add_all(&profiles, &mut report)?;

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
fn collect_profiles(path: &Path, profiles: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_dir() {
        collect_profiles_below(path, profiles)
    } else if path.is_file() {
        profiles.push(path.to_path_buf());
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{}: not found", path.display()),
        ))
    }
}

fn collect_profiles_below(dir: &Path, profiles: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_profiles_below(&path, profiles)?;
        } else if path.extension().is_some_and(|ext| ext == "slintcov") {
            profiles.push(path);
        }
    }
    Ok(())
}
