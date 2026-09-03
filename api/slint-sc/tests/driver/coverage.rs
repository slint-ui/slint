// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

//! How the driver measures the coverage of a case compiled with `--coverage`,
//! for comparing with the case's ```` ```coverage ```` block. The measuring
//! depends on how the generated code counts its coverage points; this build
//! of the compiler counts none.

use std::path::Path;

/// What a case leaves behind: the directory it ran in, its generated code,
/// and its test binary.
#[allow(dead_code)]
pub struct Case<'a> {
    pub tmp: &'a Path,
    pub generated_rs: &'a Path,
    pub test_bin: &'a Path,
}

/// A statement the test's `main` ends with.
pub const EPILOGUE: &str = "";

/// Arguments the test program is compiled with.
pub const RUSTC_ARGS: &[&str] = &[];

/// The environment the test binary runs with.
pub fn run_env(_tmp: &Path) -> Vec<(&'static str, std::path::PathBuf)> {
    Vec::new()
}

/// The coverage of the case's `.slint` files after the run.
pub fn measure(_case: &Case) -> Result<slint_sc_coverage::Report, String> {
    Err("this build of slint-compiler does not count coverage".into())
}

/// Keep what `slint-sc-coverage` needs to report the case, at `kept` (a path
/// without extension, one per case, in the coverage directory).
pub fn keep(_case: &Case, _report: &slint_sc_coverage::Report, _kept: &Path) -> Result<(), String> {
    Ok(())
}
