// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

//! Custom test driver for the Slint SC (safety-critical) subset.
//!
//! For each `.slint` file under `tests/cases/`, this driver:
//! 1. Runs `slint-compiler --slint-sc` to generate Rust code
//! 2. Extracts test code from `` ```rust `` blocks in comments
//! 3. Calls `rustc` directly to compile the generated + test code
//! 4. Runs the resulting binary
//!
//! Tests run in parallel via rayon.

use rayon::prelude::*;
use regex::Regex;
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let cases_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let test_files = collect_slint_files(&cases_dir);

    if test_files.is_empty() {
        eprintln!("No test files found in {}", cases_dir.display());
        std::process::exit(1);
    }

    let target_dir = find_target_dir();
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let instrument_coverage = std::env::var_os("LLVM_PROFILE_FILE").is_some();
    let compiler = build_compiler(&target_dir, instrument_coverage);
    let slint_sc_rlib = find_slint_sc_rlib(&target_dir);
    let rx = Regex::new(r"(?sU)\r?\n```rust\r?\n(.+)\r?\n```\r?\n").unwrap();

    let config = TestConfig {
        compiler: &compiler,
        slint_sc_rlib: &slint_sc_rlib,
        rustc: &rustc,
        instrument_coverage,
        rx: &rx,
    };

    let results: Vec<(String, Result<(), String>)> = test_files
        .par_iter()
        .map(|path| {
            let name = path
                .strip_prefix(&cases_dir)
                .unwrap_or(path)
                .with_extension("")
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            let result = run_test(path, &config);
            (name, result)
        })
        .collect();

    // Print results
    eprintln!();
    let mut failed = 0;
    for (name, result) in &results {
        match result {
            Ok(()) => eprintln!("  \x1b[32mPASS\x1b[0m {name}"),
            Err(msg) => {
                failed += 1;
                eprintln!("  \x1b[31mFAIL\x1b[0m {name}");
                for line in msg.lines() {
                    eprintln!("       {line}");
                }
            }
        }
    }

    let passed = results.len() - failed;
    eprintln!();
    eprintln!("{passed} passed, {failed} failed");

    if failed > 0 {
        std::process::exit(1);
    }
}

struct TestConfig<'a> {
    compiler: &'a Path,
    slint_sc_rlib: &'a Path,
    rustc: &'a str,
    instrument_coverage: bool,
    rx: &'a Regex,
}

fn find_target_dir() -> PathBuf {
    let self_exe = std::env::current_exe().expect("current_exe");
    self_exe
        .ancestors()
        .find(|p| p.ends_with("debug") || p.ends_with("release"))
        .expect("Could not find target dir from current_exe")
        .to_path_buf()
}

fn build_compiler(target_dir: &Path, instrument_coverage: bool) -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(&cargo);
    cmd.args(["build", "-p", "slint-compiler", "--no-default-features", "--features", "slint-sc"]);
    // Use the same target directory as the test binary itself, so the
    // compiler ends up next to us (important for cargo-llvm-cov which
    // uses a separate target dir).
    if let Some(parent) = target_dir.parent() {
        cmd.arg("--target-dir").arg(parent);
    }
    if instrument_coverage {
        cmd.env("RUSTFLAGS", "-Cinstrument-coverage");
    }
    let status = cmd.status().expect("Failed to run cargo build for slint-compiler");
    assert!(status.success(), "Failed to build slint-compiler");
    let compiler = target_dir.join("slint-compiler");
    assert!(compiler.exists(), "slint-compiler not found at {}", compiler.display());
    compiler
}

/// Find the slint-sc rlib in the deps directory for --extern.
fn find_slint_sc_rlib(target_dir: &Path) -> PathBuf {
    let deps_dir = target_dir.join("deps");
    if let Ok(entries) = std::fs::read_dir(&deps_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("libslint_sc-") && name_str.ends_with(".rlib") {
                return entry.path();
            }
        }
    }

    panic!("Could not find slint-sc rlib in {}", deps_dir.display());
}

fn run_test(slint_path: &Path, config: &TestConfig) -> Result<(), String> {
    let tmp = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let generated_rs = tmp.path().join("generated.rs");

    // Step 1: Run slint-compiler
    let output = Command::new(config.compiler)
        .arg("--slint-sc")
        .arg(slint_path)
        .arg("-o")
        .arg(&generated_rs)
        .output()
        .map_err(|e| format!("slint-compiler spawn: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("slint-compiler failed:\n{stderr}"));
    }

    // Step 2: Extract test code from ```rust blocks in comments
    let source = std::fs::read_to_string(slint_path)
        .map_err(|e| format!("read {}: {e}", slint_path.display()))?;
    let test_code = extract_rust_test_code(&source, config.rx);
    if test_code.is_empty() {
        return Err("no ```rust test code found in comments".into());
    }

    // Step 3: Create test .rs file
    let test_rs = tmp.path().join("test.rs");
    {
        let mut f = std::fs::File::create(&test_rs).map_err(|e| format!("create test.rs: {e}"))?;
        let gen_path = generated_rs.to_string_lossy().replace('\\', "/");
        let mut content = String::new();
        writeln!(content, r#"include!("{gen_path}");"#).unwrap();
        writeln!(content).unwrap();
        writeln!(content, "fn main() -> Result<(), Box<dyn std::error::Error>> {{").unwrap();
        writeln!(content, "    {}", test_code.replace('\n', "\n    ")).unwrap();
        writeln!(content, "    Ok(())").unwrap();
        writeln!(content, "}}").unwrap();
        f.write_all(content.as_bytes()).map_err(|e| format!("write test.rs: {e}"))?;
    }

    // Step 4: Compile with rustc
    let test_bin = tmp.path().join("test_bin");
    let deps_dir = config.slint_sc_rlib.parent().unwrap_or_else(|| Path::new("."));
    let mut rustc_cmd = Command::new(config.rustc);
    rustc_cmd
        .arg(&test_rs)
        .arg("--edition=2024")
        .arg("-o")
        .arg(&test_bin)
        .arg("-L")
        .arg(deps_dir)
        .arg("--extern")
        .arg(format!("slint_sc={}", config.slint_sc_rlib.display()));

    // When running under cargo-llvm-cov, propagate coverage instrumentation
    // so that slint-sc runtime code exercised by the test is included in the
    // coverage report.
    if config.instrument_coverage {
        rustc_cmd.arg("-Cinstrument-coverage");
    }

    let rustc_output = rustc_cmd.output().map_err(|e| format!("rustc spawn: {e}"))?;

    if !rustc_output.status.success() {
        let stderr = String::from_utf8_lossy(&rustc_output.stderr);
        return Err(format!("rustc failed:\n{stderr}"));
    }

    // Step 5: Run the test binary
    let run_output =
        Command::new(&test_bin).output().map_err(|e| format!("test binary spawn: {e}"))?;

    if !run_output.status.success() {
        let stderr = String::from_utf8_lossy(&run_output.stderr);
        let stdout = String::from_utf8_lossy(&run_output.stdout);
        return Err(format!("test binary failed:\nstdout: {stdout}\nstderr: {stderr}"));
    }

    Ok(())
}

fn extract_rust_test_code(source: &str, rx: &Regex) -> String {
    let mut code = String::new();
    for cap in rx.captures_iter(source) {
        if !code.is_empty() {
            code.push('\n');
        }
        code.push_str(&cap[1]);
    }
    code
}

fn collect_slint_files(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    collect_slint_files_recursive(dir, &mut results);
    results.sort();
    results
}

fn collect_slint_files_recursive(dir: &Path, results: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_slint_files_recursive(&path, results);
        } else if path.extension().is_some_and(|e| e == "slint") {
            results.push(path);
        }
    }
}
