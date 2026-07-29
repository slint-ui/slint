// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

//! Custom test driver for the Slint SC (safety-critical) subset.
//!
//! For each `.slint` file under `tests/cases/`, this driver:
//! 1. Runs `slint-compiler --slint-sc` to generate Rust code
//! 2. Extracts test code from `` ```rust `` blocks in comments
//! 3. Calls `rustc` directly to compile the generated + test code
//! 4. Runs the resulting binary; `` ```rust compile_fail `` blocks are
//!    compiled separately and must fail with every `//~ ERROR` substring in
//!    the rustc output
//! 5. Compares the screenshots taken with the `screenshot!` macro against the
//!    PNG references in `tests/references/` (set `SLINT_CREATE_SCREENSHOTS=1`
//!    to create or update them)
//!
//! Tests run in parallel via rayon.

use rayon::prelude::*;
use regex::Regex;
use std::fmt::Write as _;
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
    let rx = Regex::new(r"(?sU)\r?\n```rust( compile_fail)?\r?\n(.+)\r?\n```\r?\n").unwrap();

    let config = TestConfig {
        compiler: &compiler,
        slint_sc_rlib: &slint_sc_rlib,
        rustc: &rustc,
        instrument_coverage,
        create_screenshots: std::env::var("SLINT_CREATE_SCREENSHOTS").is_ok_and(|var| var == "1"),
        rx: &rx,
    };

    let results: Vec<(String, Result<(), String>)> = test_files
        .par_iter()
        .map(|path| {
            let rel = path.strip_prefix(&cases_dir).unwrap_or(path);
            let name =
                rel.with_extension("").to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
            let result = run_test(path, rel, &config);
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
    create_screenshots: bool,
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

fn run_test(slint_path: &Path, rel: &Path, config: &TestConfig) -> Result<(), String> {
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
    let (test_code, compile_fail_blocks) = extract_rust_test_code(&source, config.rx);
    if test_code.is_empty() {
        return Err("no ```rust test code found in comments".into());
    }
    let gen_path = generated_rs.to_string_lossy().replace('\\', "/");

    // Step 3: Create test .rs file
    let test_rs = tmp.path().join("test.rs");
    std::fs::write(&test_rs, assemble_program(&gen_path, &test_code))
        .map_err(|e| format!("write test.rs: {e}"))?;

    // Step 4: Compile with rustc
    let test_bin = tmp.path().join("test_bin");
    let rustc_output = compile(config, &test_rs, &test_bin)?;
    if !rustc_output.status.success() {
        let stderr = String::from_utf8_lossy(&rustc_output.stderr);
        return Err(format!("rustc failed:\n{stderr}"));
    }

    // The compile_fail blocks must fail to compile with the expected errors
    for (i, block) in compile_fail_blocks.iter().enumerate() {
        let expected: Vec<&str> =
            block.lines().filter_map(|l| l.trim().strip_prefix("//~ ERROR ")).collect();
        if expected.is_empty() {
            return Err(format!("compile_fail block {i} has no //~ ERROR line"));
        }
        let fail_rs = tmp.path().join(format!("compile_fail_{i}.rs"));
        std::fs::write(&fail_rs, assemble_program(&gen_path, block))
            .map_err(|e| format!("write compile_fail_{i}.rs: {e}"))?;
        let output = compile(config, &fail_rs, &tmp.path().join(format!("compile_fail_{i}")))?;
        if output.status.success() {
            return Err(format!("compile_fail block {i} compiled successfully:\n{block}"));
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        for e in expected {
            if !stderr.contains(e) {
                return Err(format!(
                    "compile_fail block {i} failed without the expected error `{e}`:\n{stderr}"
                ));
            }
        }
    }

    // Step 5: Run the test binary
    let run_output = Command::new(&test_bin)
        .current_dir(tmp.path())
        .env("SLINT_TEST_NAME", rel.file_stem().unwrap_or_default())
        .output()
        .map_err(|e| format!("test binary spawn: {e}"))?;

    if !run_output.status.success() {
        let stderr = String::from_utf8_lossy(&run_output.stderr);
        let stdout = String::from_utf8_lossy(&run_output.stdout);
        return Err(format!("test binary failed:\nstdout: {stdout}\nstderr: {stderr}"));
    }

    // Step 6: Compare the screenshots against the references
    compare_screenshots(tmp.path(), rel, config.create_screenshots)
}

/// Compare the `*.ppm` screenshots that the test binary wrote in `tmp_dir`
/// against the PNG references, which mirror the layout of the cases directory.
fn compare_screenshots(tmp_dir: &Path, rel: &Path, create: bool) -> Result<(), String> {
    let references_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/references").join(rel.parent().unwrap());
    let mut screenshots: Vec<PathBuf> = std::fs::read_dir(tmp_dir)
        .map_err(|e| format!("read_dir {}: {e}", tmp_dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "ppm"))
        .collect();
    screenshots.sort();

    let mut errors = String::new();
    for ppm_path in screenshots {
        let reference = references_dir.join(ppm_path.file_name().unwrap()).with_extension("png");
        let data =
            std::fs::read(&ppm_path).map_err(|e| format!("read {}: {e}", ppm_path.display()))?;
        let (width, height, pixels) =
            parse_ppm(&data).ok_or_else(|| format!("invalid ppm file {}", ppm_path.display()))?;
        if let Err(msg) = compare_with_reference(&reference, width, height, pixels) {
            writeln!(errors, "{}: {msg}", reference.display()).unwrap();
            if create {
                std::fs::create_dir_all(&references_dir)
                    .map_err(|e| format!("create_dir_all {}: {e}", references_dir.display()))?;
                image::save_buffer(&reference, pixels, width, height, image::ColorType::Rgb8)
                    .map_err(|e| format!("save {}: {e}", reference.display()))?;
                writeln!(
                    errors,
                    "SLINT_CREATE_SCREENSHOTS=1: wrote reference image to {}",
                    reference.display()
                )
                .unwrap();
            }
        }
    }
    // A reference for this test without a matching screenshot means the test
    // no longer takes it
    let stem = rel.file_stem().unwrap_or_default().to_string_lossy();
    for entry in std::fs::read_dir(&references_dir).into_iter().flatten().flatten() {
        let reference = entry.path();
        if reference.extension().is_none_or(|e| e != "png") {
            continue;
        }
        let ref_stem = reference.file_stem().unwrap_or_default().to_string_lossy();
        let belongs_to_test = ref_stem
            .strip_prefix(&*stem)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'));
        if belongs_to_test && !tmp_dir.join(&*ref_stem).with_extension("ppm").exists() {
            writeln!(
                errors,
                "{}: reference exists but the test did not take a screenshot named {ref_stem}; \
                 delete the file if this is intentional",
                reference.display()
            )
            .unwrap();
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn compare_with_reference(
    reference: &Path,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<(), String> {
    if !reference.exists() {
        return Err("reference is missing, run with SLINT_CREATE_SCREENSHOTS=1 to create it".into());
    }
    let img =
        image::open(reference).map_err(|e| format!("cannot read reference: {e}"))?.into_rgb8();
    if (img.width(), img.height()) != (width, height) {
        return Err(format!(
            "reference size {}x{} does not match screenshot size {width}x{height}",
            img.width(),
            img.height()
        ));
    }
    if let Some(byte) = pixels.iter().zip(img.as_raw()).position(|(a, b)| a != b) {
        let pixel = byte / 3;
        let index = pixel * 3;
        let (x, y) = (pixel as u32 % width, pixel as u32 / width);
        return Err(format!(
            "screenshot differs from reference at pixel ({x}, {y}): \
             expected #{:02x}{:02x}{:02x}, got #{:02x}{:02x}{:02x}",
            img.as_raw()[index],
            img.as_raw()[index + 1],
            img.as_raw()[index + 2],
            pixels[index],
            pixels[index + 1],
            pixels[index + 2],
        ));
    }
    Ok(())
}

/// Parse a binary PPM image as written by the `screenshot!` macro in harness.rs
fn parse_ppm(data: &[u8]) -> Option<(u32, u32, &[u8])> {
    let rest = data.strip_prefix(b"P6\n")?;
    let newline = rest.iter().position(|&b| b == b'\n')?;
    let (dimensions, rest) = rest.split_at(newline);
    let rest = rest[1..].strip_prefix(b"255\n")?;
    let (width, height) = std::str::from_utf8(dimensions).ok()?.split_once(' ')?;
    let (width, height) = (width.parse::<u32>().ok()?, height.parse::<u32>().ok()?);
    (rest.len() == width as usize * height as usize * 3).then_some((width, height, rest))
}

/// The concatenated regular test code, and each compile_fail block separately.
fn extract_rust_test_code(source: &str, rx: &Regex) -> (String, Vec<String>) {
    let mut code = String::new();
    let mut compile_fail = Vec::new();
    for cap in rx.captures_iter(source) {
        if cap.get(1).is_some() {
            compile_fail.push(cap[2].to_string());
        } else {
            if !code.is_empty() {
                code.push('\n');
            }
            code.push_str(&cap[2]);
        }
    }
    (code, compile_fail)
}

/// A test program: the generated code, the harness, and `body` as the main
/// function.
fn assemble_program(gen_path: &str, body: &str) -> String {
    let harness_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/driver/harness.rs")
        .to_string_lossy()
        .replace('\\', "/");
    let mut content = String::new();
    // no_std so that accidental use of std in the generated code doesn't compile
    writeln!(content, "#![no_std]").unwrap();
    writeln!(content, "extern crate std;").unwrap();
    writeln!(content).unwrap();
    writeln!(content, "#[macro_use]").unwrap();
    writeln!(content, r#"#[path = "{harness_path}"]"#).unwrap();
    writeln!(content, "mod harness;").unwrap();
    writeln!(content).unwrap();
    writeln!(content, r#"include!("{gen_path}");"#).unwrap();
    writeln!(content).unwrap();
    writeln!(content, "fn main() -> Result<(), std::boxed::Box<dyn std::error::Error>> {{")
        .unwrap();
    writeln!(content, "    {}", body.replace('\n', "\n    ")).unwrap();
    writeln!(content, "    Ok(())").unwrap();
    writeln!(content, "}}").unwrap();
    content
}

/// Invoke rustc on `rs_path`, linking against the slint-sc rlib.
fn compile(
    config: &TestConfig,
    rs_path: &Path,
    out_path: &Path,
) -> Result<std::process::Output, String> {
    let deps_dir = config.slint_sc_rlib.parent().unwrap_or_else(|| Path::new("."));
    let mut rustc_cmd = Command::new(config.rustc);
    rustc_cmd
        .arg(rs_path)
        .arg("--edition=2024")
        .arg("-o")
        .arg(out_path)
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

    rustc_cmd.output().map_err(|e| format!("rustc spawn: {e}"))
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
