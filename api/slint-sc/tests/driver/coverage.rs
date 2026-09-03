// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

//! How the driver measures the coverage of a case compiled with `--coverage`,
//! for comparing with the case's ```` ```coverage ```` block: the generated
//! code counts its coverage points in the `SLINT_SC_COVERAGE` static, which
//! the test's `main` writes out as a profile at its end.

use std::path::{Path, PathBuf};

/// What a case leaves behind: the directory it ran in, its generated code,
/// and its test binary.
pub struct Case<'a> {
    pub tmp: &'a Path,
    #[allow(dead_code)]
    pub generated_rs: &'a Path,
    #[allow(dead_code)]
    pub test_bin: &'a Path,
}

/// A statement the test's `main` ends with: writing the profile.
pub const EPILOGUE: &str = "crate::harness::write_coverage(&SLINT_SC_COVERAGE)?;";

/// Arguments the test program is compiled with.
pub const RUSTC_ARGS: &[&str] = &[];

/// The environment the test binary runs with.
pub fn run_env(_tmp: &Path) -> Vec<(&'static str, PathBuf)> {
    Vec::new()
}

/// The coverage of the case's `.slint` files after the run.
pub fn measure(case: &Case) -> Result<slint_sc_coverage::Report, String> {
    let profile = std::fs::read_to_string(case.tmp.join("coverage.slintcov"))
        .map_err(|e| format!("read the coverage profile: {e}"))?;
    let mut report = slint_sc_coverage::Report::default();
    slint_sc_coverage::profile::add_all(
        &[slint_sc_coverage::profile::parse(&profile)?],
        &mut report,
    )?;
    Ok(report)
}

/// Keep what `slint-sc-coverage` needs to report the case, at `kept` (a path
/// without extension, one per case, in the coverage directory): the profile.
pub fn keep(case: &Case, _report: &slint_sc_coverage::Report, kept: &Path) -> Result<(), String> {
    let profile = kept.with_extension("slintcov");
    std::fs::copy(case.tmp.join("coverage.slintcov"), &profile)
        .map_err(|e| format!("copy the coverage profile to {}: {e}", profile.display()))?;
    Ok(())
}
