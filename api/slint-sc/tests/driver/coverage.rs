// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

//! How the driver measures the coverage of a case's `.slint` code, for
//! comparing with what the case states: the compiler maps the coverage
//! points to ranges of the generated code, LLVM measures the code when the
//! test program is built with `-C instrument-coverage`, and the count of
//! the code of a point's range is the point's.

use slint_sc_coverage::source_map;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The profile the case's test binary writes, in the case's directory.
/// Under cargo-llvm-cov, it follows the pattern cargo-llvm-cov gathers the
/// profiles by, with the case's directory in place of every specifier (the
/// process and binary ids), so the runtime code the case exercises is in
/// the runtime's coverage too.
pub fn profile(tmp: &Path) -> PathBuf {
    let unique = tmp.file_name().unwrap_or_default().to_string_lossy().replace('.', "");
    match std::env::var("LLVM_PROFILE_FILE") {
        Ok(pattern) => {
            let specifier = regex::Regex::new(r"%\d*[A-Za-z]").unwrap();
            PathBuf::from(specifier.replace_all(&pattern, unique.as_str()).into_owned())
        }
        Err(_) => tmp.join("case.profraw"),
    }
}

/// The coverage of the case's `.slint` files after the run of `test_bin`,
/// built from `generated_rs` in the case's directory `tmp`.
pub fn measure(
    tmp: &Path,
    generated_rs: &Path,
    test_bin: &Path,
) -> Result<slint_sc_coverage::Report, String> {
    let generated = generated_rs.canonicalize().map_err(|e| e.to_string())?;
    let export = source_map::export_coverage(
        &[test_bin.to_path_buf()],
        &[profile(tmp)],
        std::slice::from_ref(&generated),
    )?;
    let mut regions = source_map::Regions::default();
    regions.parse(&export, tmp)?;
    let file_regions = regions.files.get(&generated).ok_or("the export names no generated code")?;
    let map = std::fs::read_to_string(generated.with_extension(source_map::MAP_EXTENSION))
        .map_err(|e| format!("read the coverage map: {e}"))?;
    let mut report = slint_sc_coverage::Report::default();
    if source_map::add(&map, file_regions, &mut report)? == 0 {
        return Err("no coverage point in the map".into());
    }
    Ok(report)
}

/// Keep the case's coverage as lcov at `kept` (a path without extension,
/// one per case), the paths relative to the repository: the code the
/// binary's coverage refers to is gone with the case's directory.
pub fn keep(report: &slint_sc_coverage::Report, kept: &Path) -> Result<(), String> {
    static REPOSITORY: OnceLock<PathBuf> = OnceLock::new();
    let repository = REPOSITORY.get_or_init(|| {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        repository.canonicalize().unwrap_or(repository)
    });
    std::fs::write(kept.with_extension("lcov"), report.lcov(repository)).map_err(|e| e.to_string())
}
