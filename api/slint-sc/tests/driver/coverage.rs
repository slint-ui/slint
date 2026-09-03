// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

//! How the driver measures the coverage of a case compiled with `--coverage`,
//! for comparing with the case's ```` ```coverage ```` block: the generated
//! code calls a marker function where a point is reached, which LLVM counts
//! when the test program is built with `-C instrument-coverage`, and the
//! `.slintcov` map next to the generated code locates the points.

use slint_sc_coverage::markers;
use std::path::{Path, PathBuf};

/// What a case leaves behind: the directory it ran in, its generated code,
/// and its test binary.
pub struct Case<'a> {
    pub tmp: &'a Path,
    pub generated_rs: &'a Path,
    #[allow(dead_code)]
    pub test_bin: &'a Path,
}

/// A statement the test's `main` ends with.
pub const EPILOGUE: &str = "";

/// Arguments the test program is compiled with.
pub const RUSTC_ARGS: &[&str] = &["-Cinstrument-coverage"];

/// The environment the test binary runs with: where to write its profile.
pub fn run_env(tmp: &Path) -> Vec<(&'static str, PathBuf)> {
    vec![("LLVM_PROFILE_FILE", profile(tmp))]
}

/// The profile of the case. Under cargo-llvm-cov, it goes where cargo-llvm-cov
/// gathers the profiles from, named after the case's directory in place of
/// the process and binary ids, so the runtime code the case exercises is in
/// the runtime's coverage too.
fn profile(tmp: &Path) -> PathBuf {
    let unique = tmp.file_name().unwrap_or_default().to_string_lossy().replace('.', "");
    match std::env::var("LLVM_PROFILE_FILE") {
        Ok(pattern) => PathBuf::from(pattern.replace("%p", &unique).replace("%m", "case")),
        Err(_) => tmp.join("case.profraw"),
    }
}

/// The coverage of the case's `.slint` files after the run.
pub fn measure(case: &Case) -> Result<slint_sc_coverage::Report, String> {
    let map = std::fs::read_to_string(case.generated_rs.with_extension("slintcov"))
        .map_err(|e| format!("read the coverage map: {e}"))?;
    let map = markers::parse_map(&map)?;
    let counts = markers::parse_profile(&markers::merge_profiles(&[profile(case.tmp)])?);
    let mut report = slint_sc_coverage::Report::default();
    if !markers::add(&map, &counts, &mut report) {
        return Err("the profile has none of the case's coverage markers".into());
    }
    Ok(report)
}

/// Keep what `slint-sc-coverage` needs to report the case, at `kept` (a path
/// without extension, one per case, in the coverage directory): the map and,
/// outside cargo-llvm-cov, the profile.
pub fn keep(case: &Case, _report: &slint_sc_coverage::Report, kept: &Path) -> Result<(), String> {
    let copy = |from: PathBuf, to: PathBuf| {
        std::fs::copy(&from, &to).map_err(|e| format!("copy to {}: {e}", to.display()))
    };
    copy(case.generated_rs.with_extension("slintcov"), kept.with_extension("slintcov"))?;
    if std::env::var_os("LLVM_PROFILE_FILE").is_none() {
        copy(profile(case.tmp), kept.with_extension("profraw"))?;
    }
    Ok(())
}
