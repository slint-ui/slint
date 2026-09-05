// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

//! How the driver measures the coverage of a case, for comparing with the
//! case's ```` ```coverage ```` block: the compiler maps the coverage points
//! to ranges of the generated code, LLVM measures the code when the test
//! program is built with `-C instrument-coverage`, and the count of the code
//! of a point's range is the point's.

use slint_sc_coverage::source_map;
use std::path::{Path, PathBuf};

/// What a case leaves behind: the directory it ran in, its generated code,
/// and its test binary.
pub struct Case<'a> {
    pub tmp: &'a Path,
    pub generated_rs: &'a Path,
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

/// The profile of the case. Under cargo-llvm-cov, it follows the pattern
/// cargo-llvm-cov gathers the profiles by, with the case's directory in
/// place of the process and binary ids, so the runtime code the case
/// exercises is in the runtime's coverage too.
fn profile(tmp: &Path) -> PathBuf {
    let unique = tmp.file_name().unwrap_or_default().to_string_lossy().replace('.', "");
    match std::env::var("LLVM_PROFILE_FILE") {
        Ok(pattern) => PathBuf::from(pattern.replace("%p", &unique).replace("%m", "case")),
        Err(_) => tmp.join("case.profraw"),
    }
}

/// The coverage of the case's `.slint` files after the run.
pub fn measure(case: &Case) -> Result<slint_sc_coverage::Report, String> {
    let export = source_map::export_coverage(&[case.test_bin.to_path_buf()], &[profile(case.tmp)])?;
    let mut regions = source_map::Regions::default();
    regions.parse(&export, case.tmp)?;
    let generated = case.generated_rs.canonicalize().map_err(|e| e.to_string())?;
    let file_regions = regions.files.get(&generated).ok_or("the export names no generated code")?;
    let map = std::fs::read_to_string(generated.with_extension("slintcov"))
        .map_err(|e| format!("read the coverage map: {e}"))?;
    let mut report = slint_sc_coverage::Report::default();
    if source_map::add(&map, file_regions, &mut report)? == 0 {
        return Err("no coverage point in the map".into());
    }
    Ok(report)
}

/// Keep what `slint-sc-coverage` needs to report the case, at `kept` (a path
/// without extension, one per case, in the coverage directory): the case's
/// coverage as lcov, the paths relative to the repository, as the code the
/// binary's coverage refers to is gone with the case's directory.
pub fn keep(_case: &Case, report: &slint_sc_coverage::Report, kept: &Path) -> Result<(), String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let repository = repository.canonicalize().map_err(|e| e.to_string())?;
    let lcov = report.lcov(&repository);
    std::fs::write(kept.with_extension("lcov"), lcov).map_err(|e| e.to_string())
}
