// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use rayon::prelude::*;
use std::path::Path;
use std::process::Command;

// To add a new fixture: drop `<name>.slint` and `<name>.expected.pot` into
// tests/fixtures/, then append the name here.
const CASES: &[&str] = &["sample"];

// Normalize a .pot file for comparison:
// - strip the REUSE/SPDX header that reference files carry (but the extractor doesn't emit)
// - redact the POT-Creation-Date value, which changes every run
fn normalize(pot: &str) -> String {
    let mut lines = pot.lines().peekable();
    while let Some(line) = lines.peek() {
        if line.starts_with("# Copyright") || line.starts_with("# SPDX-") || line.is_empty() {
            lines.next();
        } else {
            break;
        }
    }
    let mut out = String::with_capacity(pot.len());
    for line in lines {
        if line.starts_with("\"POT-Creation-Date:") {
            out.push_str("\"POT-Creation-Date: REDACTED\\n\"");
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

fn run_case(fixtures: &Path, name: &str) -> Result<(), String> {
    let bin = env!("CARGO_BIN_EXE_slint-tr-extractor");
    let tmp = tempfile::tempdir().map_err(|e| format!("[{name}] tempdir: {e}"))?;
    let out = tmp.path().join(format!("{name}.pot"));
    let input = format!("{name}.slint");

    let status = Command::new(bin)
        .current_dir(fixtures)
        .arg("-o")
        .arg(&out)
        .arg(&input)
        .status()
        .map_err(|e| format!("[{name}] spawn: {e}"))?;
    if !status.success() {
        return Err(format!("[{name}] extractor exited with {status}"));
    }

    let actual = normalize(
        &std::fs::read_to_string(&out).map_err(|e| format!("[{name}] read output: {e}"))?,
    );
    let expected_path = fixtures.join(format!("{name}.expected.pot"));
    let expected = normalize(
        &std::fs::read_to_string(&expected_path)
            .map_err(|e| format!("[{name}] read {}: {e}", expected_path.display()))?,
    );

    if actual != expected {
        return Err(format!(
            "[{name}] generated .pot does not match reference\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
        ));
    }
    Ok(())
}

#[test]
fn pot_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures");

    let failures: Vec<String> =
        CASES.par_iter().filter_map(|name| run_case(&fixtures, name).err()).collect();

    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
