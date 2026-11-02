// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

// cSpell: ignore defsym nographic

//! Custom test driver for the Slint SC (safety-critical) subset.
//!
//! For each test case, a `.slint` file in a group directory of `tests/cases/`,
//! this driver:
//! 1. Runs `slint-compiler --slint-sc` to generate Rust code
//! 2. Extracts test code from `` ```rust `` blocks in comments
//! 3. Calls `rustc` directly to compile the generated + test code, and
//!    `clippy-driver` to lint the generated code
//! 4. Runs the resulting binary; `` ```rust compile_fail `` blocks are
//!    compiled separately and must fail with every `//~ ERROR` substring in
//!    the rustc output
//! 5. Compares the screenshots taken with the `screenshot!` macro against the
//!    PNG references in `tests/references/` (set `SLINT_CREATE_SCREENSHOTS=1`
//!    to create or update them)
//! 6. Measures the coverage of the case's `.slint` code, the test program
//!    being built with coverage instrumentation, and compares it with what
//!    the case states in its `//#c` caret lines, if it has any
//!    (see `slint_sc_coverage::expectations`; set
//!    `SLINT_COVERAGE_TEST_UPDATE=1` to rewrite them from the measurement;
//!    the case still fails that run).
//!    With `SLINT_SC_COVERAGE_DIR` set, keeps each case's coverage there
//!
//! Tests run in parallel via rayon.
//!
//! With `SLINT_SC_TARGET` set to one of [`TARGETS`], the test programs build
//! for that target and run in QEMU.
//! The lints, the `compile_fail` blocks, and the coverage are then left out,
//! since they don't depend on the target.

#[path = "driver/coverage.rs"]
mod coverage;

use rayon::prelude::*;
use regex::Regex;
use slint_sc_coverage::expectations;
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
    let clippy_driver = std::env::var("CLIPPY_DRIVER").unwrap_or_else(|_| "clippy-driver".into());
    // Where each case's coverage (see the `coverage` module) is kept.
    let coverage_dir = std::env::var_os("SLINT_SC_COVERAGE_DIR").map(PathBuf::from);
    let update_coverage = std::env::var(expectations::UPDATE_VAR).is_ok_and(|var| var == "1");
    let target = std::env::var("SLINT_SC_TARGET").ok().map(|triple| {
        TARGETS.iter().find(|target| target.triple == triple).unwrap_or_else(|| {
            let known: Vec<_> = TARGETS.iter().map(|target| target.triple).collect();
            panic!("SLINT_SC_TARGET={triple} isn't one of {}", known.join(", "))
        })
    });
    let compiler = build_compiler(&target_dir);
    let slint_sc_rlib = build_slint_sc_rlib(&target_dir, target);
    let rx = Regex::new(r"(?sU)\r?\n```rust( compile_fail)?\r?\n(.+)\r?\n```\r?\n").unwrap();

    let config = TestConfig {
        compiler: &compiler,
        slint_sc_rlib: &slint_sc_rlib,
        rustc: &rustc,
        clippy_driver: &clippy_driver,
        target,
        coverage_dir: coverage_dir.as_deref(),
        update_coverage,
        create_screenshots: std::env::var("SLINT_CREATE_SCREENSHOTS").is_ok_and(|var| var == "1"),
        rx: &rx,
    };

    let mut results: Vec<(String, Result<(), String>)> = test_files
        .par_iter()
        .map(|path| {
            let rel = path.strip_prefix(&cases_dir).unwrap_or(path);
            let name =
                rel.with_extension("").to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
            let result = run_test(path, rel, &config);
            (name, result)
        })
        .collect();

    if target.is_none() {
        results.push(("version-check".into(), run_version_check(&config)));
    }

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

    if let Some(path) = std::env::var_os("SLINT_TEST_REPORT") {
        let outcomes: Vec<(String, String, bool)> = results
            .iter()
            // The repository-relative source of each case, for linking.
            .map(|(name, result)| {
                (name.clone(), format!("api/slint-sc/tests/cases/{name}.slint"), result.is_ok())
            })
            .collect();
        write_report(&outcomes, "slint-sc-driver", Path::new(&path))
            .unwrap_or_else(|e| panic!("failed to write test report: {e}"));
    }

    if failed > 0 {
        std::process::exit(1);
    }
}

/// Write the per-case `(name, source path, passed)` results as CTRF-style
/// JSON, for the safety manual's Test Results page.
fn write_report(
    results: &[(String, String, bool)],
    tool: &str,
    path: &std::path::Path,
) -> std::io::Result<()> {
    let tests: Vec<_> = results
        .iter()
        .map(|(name, file_path, ok)| {
            serde_json::json!({
                "name": name,
                "filePath": file_path,
                "status": if *ok { "passed" } else { "failed" },
            })
        })
        .collect();
    let failed = results.iter().filter(|(_, _, ok)| !ok).count();
    let report = serde_json::json!({
        "results": {
            "tool": { "name": tool },
            "summary": {
                "tests": results.len(),
                "passed": results.len() - failed,
                "failed": failed,
            },
            "tests": tests,
        }
    });
    std::fs::write(path, serde_json::to_string_pretty(&report).unwrap())
}

struct TestConfig<'a> {
    compiler: &'a Path,
    slint_sc_rlib: &'a Path,
    rustc: &'a str,
    clippy_driver: &'a str,
    target: Option<&'a Target>,
    /// Where each case's coverage is kept, when it is.
    coverage_dir: Option<&'a Path>,
    /// Rewrite what a case states about its coverage from the measurement.
    update_coverage: bool,
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

/// A `cargo build` into the same target directory as the test binary itself,
/// so the artifacts end up next to us (important for cargo-llvm-cov which
/// uses a separate target dir).
fn cargo_build(target_dir: &Path) -> Command {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(&cargo);
    cmd.arg("build");
    if let Some(parent) = target_dir.parent() {
        cmd.arg("--target-dir").arg(parent);
    }
    cmd
}

fn build_compiler(target_dir: &Path) -> PathBuf {
    let mut cmd = cargo_build(target_dir);
    cmd.args(["-p", "slint-compiler", "--no-default-features", "--features", "slint-sc"]);
    // Coverage measures the slint-sc runtime alone, so the compiler builds
    // uninstrumented: clear the rustc wrapper cargo-llvm-cov injects the
    // instrumentation with, and the flag variables older versions used.
    cmd.env_remove("RUSTC_WRAPPER");
    cmd.env_remove("RUSTFLAGS");
    cmd.env_remove("CARGO_ENCODED_RUSTFLAGS");
    let status = cmd.status().expect("Failed to run cargo build for slint-compiler");
    assert!(status.success(), "Failed to build slint-compiler");
    let compiler = target_dir.join(format!("slint-compiler{}", std::env::consts::EXE_SUFFIX));
    assert!(compiler.exists(), "slint-compiler not found at {}", compiler.display());
    compiler
}

/// Cargo places the library of the package it builds in the target directory.
/// The environment is kept, so under cargo-llvm-cov this is the instrumented
/// build that the test binary links.
fn build_slint_sc_rlib(target_dir: &Path, target: Option<&Target>) -> PathBuf {
    let mut cmd = cargo_build(target_dir);
    cmd.args(["-p", "slint-sc", "--lib"]);
    if let Some(target) = target {
        cmd.args(["--target", target.triple]);
        if let Platform::BareMetal { .. } = target.platform {
            cmd.args(["-p", "slint-sc-test-sys"]);
        }
    }
    let rlib_dir = match target {
        // Cargo builds for a `--target` in a directory of its own
        Some(target) => target_dir.parent().unwrap().join(target.triple).join("debug"),
        None => target_dir.to_path_buf(),
    };
    let status = cmd.status().expect("Failed to run cargo build for slint-sc");
    assert!(status.success(), "Failed to build slint-sc");
    let rlib = rlib_dir.join("libslint_sc.rlib");
    assert!(rlib.exists(), "slint-sc rlib not found at {}", rlib.display());
    rlib
}

fn run_test(slint_path: &Path, rel: &Path, config: &TestConfig) -> Result<(), String> {
    let tmp = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let generated_rs = tmp.path().join("generated.rs");

    // Step 1: Run slint-compiler
    let mut compiler = Command::new(config.compiler);
    compiler.arg("--slint-sc").arg(slint_path).arg("-o").arg(&generated_rs);
    if config.target.is_none() {
        compiler.arg("--coverage");
    }
    let output = compiler.output().map_err(|e| format!("slint-compiler spawn: {e}"))?;

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
    let name = rel.file_stem().unwrap_or_default().to_string_lossy();
    std::fs::write(&test_rs, assemble_program(&gen_path, &test_code, &name))
        .map_err(|e| format!("write test.rs: {e}"))?;

    // Step 4: Compile with rustc
    let test_bin = tmp.path().join(format!("test_bin{}", std::env::consts::EXE_SUFFIX));
    let rustc_output = compile(config, &test_rs, &test_bin)?;
    if !rustc_output.status.success() {
        let stderr = String::from_utf8_lossy(&rustc_output.stderr);
        return Err(format!("rustc failed:\n{stderr}"));
    }

    let on_host = config.target.is_none();
    if on_host {
        lint(config, &test_rs, tmp.path())?;
        check_compile_fail(config, &compile_fail_blocks, &gen_path, &name, tmp.path())?;
    }

    // Step 5: Run the test binary
    run(config, &test_bin, tmp.path())?;

    if on_host {
        check_coverage(config, slint_path, rel, &source, &generated_rs, &test_bin, tmp.path())?;
    }

    // Step 7: Compare the screenshots against the references
    compare_screenshots(tmp.path(), rel, config.create_screenshots)
}

fn check_compile_fail(
    config: &TestConfig,
    blocks: &[String],
    gen_path: &str,
    name: &str,
    tmp: &Path,
) -> Result<(), String> {
    // The compile_fail blocks must fail to compile with the expected errors
    for (i, block) in blocks.iter().enumerate() {
        let expected: Vec<&str> =
            block.lines().filter_map(|l| l.trim().strip_prefix("//~ ERROR ")).collect();
        if expected.is_empty() {
            return Err(format!("compile_fail block {i} has no //~ ERROR line"));
        }
        let fail_rs = tmp.join(format!("compile_fail_{i}.rs"));
        std::fs::write(&fail_rs, assemble_program(gen_path, block, name))
            .map_err(|e| format!("write compile_fail_{i}.rs: {e}"))?;
        let output = compile(config, &fail_rs, &tmp.join(format!("compile_fail_{i}")))?;
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
    Ok(())
}

fn check_coverage(
    config: &TestConfig,
    slint_path: &Path,
    rel: &Path,
    source: &str,
    generated_rs: &Path,
    test_bin: &Path,
    tmp: &Path,
) -> Result<(), String> {
    // Step 6: The coverage of the case must be what the case states, if it
    // does: every point, reached or not.
    let report = coverage::measure(tmp, generated_rs, test_bin)?;
    // The cases of the `coverage` group test only the reporting: one that lost its
    // caret lines would pass for stating nothing.
    if rel.starts_with("coverage") && !expectations::is_stated(source) {
        return Err("a coverage case states its coverage in `//#c` caret lines".into());
    }
    // The case is rewritten when asked to, and the difference is still a
    // failure, so that an update never passes unseen.
    if let Err(difference) = expectations::check(source, slint_path, &report) {
        if !config.update_coverage {
            return Err(difference);
        }
        let updated = expectations::update(source, slint_path, &report)?;
        std::fs::write(slint_path, updated).map_err(|e| format!("rewrite the case: {e}"))?;
        return Err(format!("{difference}\nthe case was rewritten"));
    }
    if let Some(dir) = config.coverage_dir {
        let kept = dir.join(rel);
        std::fs::create_dir_all(kept.parent().unwrap()).map_err(|e| format!("mkdir: {e}"))?;
        coverage::keep(&report, &kept)?;
    }
    Ok(())
}

/// Check that the generated code compiles only against the slint-sc runtime of
/// the compiler's own version.
///
/// The compiler stamps the version it was built with into the generated code,
/// and this test binary is part of the slint-sc crate, so `CARGO_PKG_VERSION`
/// here is the runtime's version. The generated code carrying that same version
/// is what makes the two agree; a reference to any other version fails to
/// compile against the runtime.
//#sls.gen.version
fn run_version_check(config: &TestConfig) -> Result<(), String> {
    let tmp = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let version = env!("CARGO_PKG_VERSION").replace('.', "_");

    // The generated code is stamped with the runtime's version.
    let slint = tmp.path().join("version.slint");
    std::fs::write(&slint, "export component Foo inherits Window {}\n")
        .map_err(|e| format!("write version.slint: {e}"))?;
    let generated = tmp.path().join("generated.rs");
    let output = Command::new(config.compiler)
        .arg("--slint-sc")
        .arg(&slint)
        .arg("-o")
        .arg(&generated)
        .output()
        .map_err(|e| format!("slint-compiler spawn: {e}"))?;
    if !output.status.success() {
        return Err(format!("slint-compiler failed:\n{}", String::from_utf8_lossy(&output.stderr)));
    }
    let generated =
        std::fs::read_to_string(&generated).map_err(|e| format!("read generated: {e}"))?;
    let expected = format!("VersionCheck_{version}");
    if !generated.contains(&expected) {
        return Err(format!("generated code is not stamped with `{expected}`:\n{generated}"));
    }

    // A reference to any other version does not compile against the runtime.
    let mismatch = tmp.path().join("mismatch.rs");
    std::fs::write(
        &mismatch,
        "fn main() {}\nconst _: slint_sc::VersionCheck_0_0_0 = slint_sc::VersionCheck_0_0_0;\n",
    )
    .map_err(|e| format!("write mismatch.rs: {e}"))?;
    let output = compile(config, &mismatch, &tmp.path().join("mismatch"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() || !stderr.contains("VersionCheck_0_0_0") {
        return Err(format!(
            "a reference to a different runtime version did not fail to build:\n{stderr}"
        ));
    }

    Ok(())
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

/// The lints denied for the generated code only. The harness and the test
/// bodies don't ship, and report a failure by panicking, so the program
/// allows them there.
const GENERATED_CODE_LINTS: &[&str] = &[
    //#sls.gen.no-shadow
    "clippy::shadow_same",
    "clippy::shadow_reuse",
    "clippy::shadow_unrelated",
    //#sls.gen.no-panic
    "clippy::unwrap_used",
    "clippy::expect_used",
    "clippy::panic",
    "clippy::unreachable",
    "clippy::todo",
    "clippy::unimplemented",
    //#sls.gen.no-indexing
    "clippy::indexing_slicing",
    //#sls.gen.no-question-mark
    "clippy::question_mark_used",
    //#sls.gen.no-overflow
    "clippy::arithmetic_side_effects",
    //#sls.gen.no-as
    "clippy::as_conversions",
];

/// A test program: the generated code, the harness, and `body` as the main
/// function.
fn assemble_program(gen_path: &str, body: &str, name: &str) -> String {
    let harness_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/driver/harness.rs")
        .to_string_lossy()
        .replace('\\', "/");
    let mut content = String::new();
    // Only the harness declares `extern crate std`, so the generated code at
    // the crate root can name neither `std` nor `alloc`
    //#sls.gen.no-std
    writeln!(content, "#![no_std]").unwrap();
    // On a bare-metal target, the harness defines the entry point
    writeln!(content, "#![cfg_attr(target_os = \"none\", no_main)]").unwrap();
    //#sls.gen.no-unsafe
    writeln!(content, "#![forbid(unsafe_code)]").unwrap();
    let generated_code_lints = GENERATED_CODE_LINTS.join(", ");
    writeln!(content, "#![deny({generated_code_lints})]").unwrap();
    writeln!(content).unwrap();
    // The name of the screenshots, see `screenshot!`
    writeln!(content, "macro_rules! test_name {{ () => {{ {name:?} }} }}").unwrap();
    writeln!(content, "#[allow({generated_code_lints})]").unwrap();
    writeln!(content, "#[macro_use]").unwrap();
    writeln!(content, r#"#[path = "{harness_path}"]"#).unwrap();
    writeln!(content, "mod harness;").unwrap();
    writeln!(content).unwrap();
    writeln!(content, r#"include!("{gen_path}");"#).unwrap();
    writeln!(content).unwrap();
    writeln!(content, "#[allow({generated_code_lints})]").unwrap();
    writeln!(content, "fn main() -> Result<(), harness::Error> {{").unwrap();
    writeln!(content, "    {}", body.replace('\n', "\n    ")).unwrap();
    writeln!(content, "    Ok(())").unwrap();
    writeln!(content, "}}").unwrap();
    content
}

/// Run clippy on the test program at `rs_path`, which denies the lints of
/// [`assemble_program`].
fn lint(config: &TestConfig, rs_path: &Path, tmp_dir: &Path) -> Result<(), String> {
    let output = rust_command(config.clippy_driver, config, rs_path)
        .args(["--emit=metadata", "--out-dir"])
        .arg(tmp_dir)
        .output()
        .map_err(|e| format!("{} spawn: {e}", config.clippy_driver))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("clippy failed:\n{stderr}"));
    }
    Ok(())
}

/// Invoke rustc on `rs_path`, linking against the slint-sc rlib.
fn compile(
    config: &TestConfig,
    rs_path: &Path,
    out_path: &Path,
) -> Result<std::process::Output, String> {
    let mut rustc_cmd = rust_command(config.rustc, config, rs_path);
    match config.target.map(|target| &target.platform) {
        None => {
            // Instrumented for the coverage of the case's .slint code (see the
            // `coverage` module); under cargo-llvm-cov, the runtime code the case
            // exercises is in the runtime's coverage too.
            rustc_cmd.arg("-Cinstrument-coverage");
        }
        Some(Platform::Linux { linker, .. }) => {
            rustc_cmd.arg(format!("-Clinker={linker}"));
        }
        Some(Platform::BareMetal { code, ram, .. }) => {
            // The host build checks that the generated code uses no crate but
            // slint-sc, see `rust_command`.
            let rlib_dir = config.slint_sc_rlib.parent().unwrap();
            let mut dependencies = std::ffi::OsString::from("dependency=");
            dependencies.push(rlib_dir.join("deps"));
            rustc_cmd.args(["--extern", "slint_sc_test_sys", "-L"]).arg(rlib_dir);
            rustc_cmd.arg("-L").arg(&dependencies);
            let link_x =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/driver/bare_metal/link.x");
            rustc_cmd.arg(format!("-Clink-arg=-T{}", link_x.display()));
            rustc_cmd.arg(format!("-Clink-arg=--defsym=CODE={code}"));
            rustc_cmd.arg(format!("-Clink-arg=--defsym=RAM={ram}"));
        }
    }
    rustc_cmd.arg("-o").arg(out_path);
    rustc_cmd.output().map_err(|e| format!("rustc spawn: {e}"))
}

/// A `program` command, rustc or clippy-driver, for the test program at `rs_path`.
fn rust_command(program: &str, config: &TestConfig, rs_path: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.arg(rs_path)
        .arg("--edition=2024")
        // slint-sc is the only `--extern`, so the generated code fails to build
        // if it references any other crate.
        //#sls.gen.output
        .arg("--extern")
        .arg(format!("slint_sc={}", config.slint_sc_rlib.display()));
    if let Some(target) = config.target {
        cmd.arg(format!("--target={}", target.triple));
    }
    // The generated code must build on stable, even when the suite enables
    // unstable options for the runtime's branch coverage.
    cmd.env_remove("RUSTC_BOOTSTRAP");
    cmd
}

/// How long a test program may run. A bare-metal program that faults loops
/// forever.
const RUN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Run the test program `test_bin` in `tmp_dir`, where it writes its
/// screenshots and, on the host, its coverage profile.
fn run(config: &TestConfig, test_bin: &Path, tmp_dir: &Path) -> Result<(), String> {
    let mut run = match config.target.map(|target| &target.platform) {
        None => {
            let mut run = Command::new(test_bin);
            run.env("LLVM_PROFILE_FILE", coverage::profile(tmp_dir));
            run
        }
        Some(Platform::Linux { runner, .. }) => match runner.split_first() {
            Some((program, args)) => {
                let mut run = Command::new(program);
                run.args(args).arg(test_bin);
                run
            }
            None => Command::new(test_bin),
        },
        Some(Platform::BareMetal { qemu, machine, .. }) => {
            let mut run = Command::new(qemu);
            run.args(*machine).args(QEMU_SEMIHOSTING).arg(test_bin);
            run
        }
    };
    // The output goes to files: a pipe could fill up while waiting
    let stdout_path = tmp_dir.join("stdout.txt");
    let stderr_path = tmp_dir.join("stderr.txt");
    let file = |path: &Path| std::fs::File::create(path).map_err(|e| format!("create output: {e}"));
    run.current_dir(tmp_dir).stdout(file(&stdout_path)?).stderr(file(&stderr_path)?);
    let mut child = run.spawn().map_err(|e| format!("test binary spawn: {e}"))?;
    let start = std::time::Instant::now();
    let outcome = loop {
        if let Some(status) = child.try_wait().map_err(|e| format!("test binary wait: {e}"))? {
            if status.success() {
                return Ok(());
            }
            break format!("failed ({status})");
        }
        if start.elapsed() > RUN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            break format!("timed out after {RUN_TIMEOUT:?}");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let stdout = std::fs::read_to_string(&stdout_path).unwrap_or_default();
    let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
    Err(format!("test binary {outcome}:\nstdout: {stdout}\nstderr: {stderr}"))
}

/// A target the test programs build for and run on in place of the host.
struct Target {
    triple: &'static str,
    platform: Platform,
}

enum Platform {
    /// A Linux target, with the standard library
    Linux {
        linker: &'static str,
        /// The command that runs the test program, the program appended;
        /// none for a target the host runs
        runner: &'static [&'static str],
    },
    /// A target without operating system, with the platform of
    /// `bare_metal/lib.rs`
    BareMetal {
        /// The QEMU program, and the options of a machine of the target
        qemu: &'static str,
        machine: &'static [&'static str],
        /// The addresses of the program's code and data, in the machine's
        /// memory; see `bare_metal/link.x`
        code: &'static str,
        ram: &'static str,
    },
}

/// The options of QEMU for a bare-metal test program, the program appended.
const QEMU_SEMIHOSTING: &[&str] = &[
    "-nographic",
    "-monitor",
    "none",
    "-serial",
    "none",
    "-semihosting-config",
    "enable=on,target=native",
    "-kernel",
];

/// The targets of Ferrocene the test programs build for and run on, other
/// than the hosts. The Linux targets with glibc use the cross compilers and
/// libraries of Debian and Ubuntu.
const TARGETS: &[Target] = &[
    Target {
        triple: "x86_64-unknown-linux-musl",
        platform: Platform::Linux { linker: "rust-lld", runner: &[] },
    },
    Target {
        triple: "aarch64-unknown-linux-musl",
        platform: Platform::Linux { linker: "rust-lld", runner: &["qemu-aarch64"] },
    },
    Target {
        triple: "riscv64gc-unknown-linux-gnu",
        platform: Platform::Linux {
            linker: "riscv64-linux-gnu-gcc",
            runner: &["qemu-riscv64", "-L", "/usr/riscv64-linux-gnu"],
        },
    },
    Target {
        triple: "powerpc64le-unknown-linux-gnu",
        platform: Platform::Linux {
            linker: "powerpc64le-linux-gnu-gcc",
            runner: &["qemu-ppc64le", "-L", "/usr/powerpc64le-linux-gnu"],
        },
    },
    Target {
        triple: "s390x-unknown-linux-gnu",
        platform: Platform::Linux {
            linker: "s390x-linux-gnu-gcc",
            runner: &["qemu-s390x", "-L", "/usr/s390x-linux-gnu"],
        },
    },
    Target {
        triple: "aarch64-unknown-none",
        platform: Platform::BareMetal {
            qemu: "qemu-system-aarch64",
            // No network card, which would need a ROM
            machine: &["-machine", "virt", "-cpu", "cortex-a53", "-nic", "none"],
            code: "0x40000000",
            ram: "0x40800000",
        },
    },
    // QEMU has no Armv7-R machine that runs a program without firmware; the
    // Armv8-R Cortex-R52 runs Armv7-R code
    Target { triple: "armv7r-none-eabihf", platform: MPS3_AN536 },
    Target { triple: "armv8r-none-eabihf", platform: MPS3_AN536 },
    // The Armv7E-M Cortex-M4 runs Armv6-M code
    Target { triple: "thumbv6m-none-eabi", platform: MPS2_AN386 },
    Target { triple: "thumbv7em-none-eabi", platform: MPS2_AN386 },
    Target { triple: "thumbv7em-none-eabihf", platform: MPS2_AN386 },
    // The Armv8-M Mainline Cortex-M33 runs Armv8-M Baseline code
    Target { triple: "thumbv8m.base-none-eabi", platform: MPS2_AN505 },
    Target { triple: "thumbv8m.main-none-eabi", platform: MPS2_AN505 },
    Target { triple: "thumbv8m.main-none-eabihf", platform: MPS2_AN505 },
];

const MPS2_AN386: Platform = Platform::BareMetal {
    qemu: "qemu-system-arm",
    machine: &["-machine", "mps2-an386"],
    code: "0x0",
    ram: "0x20000000",
};
/// The Cortex-M33 starts in the secure state, so the program is at the
/// secure aliases of the memory
const MPS2_AN505: Platform = Platform::BareMetal {
    qemu: "qemu-system-arm",
    machine: &["-machine", "mps2-an505"],
    code: "0x10000000",
    ram: "0x38000000",
};
const MPS3_AN536: Platform = Platform::BareMetal {
    qemu: "qemu-system-arm",
    machine: &["-machine", "mps3-an536"],
    code: "0x20000000",
    ram: "0x20800000",
};

/// The cases are the `.slint` files one level below `dir`, in a group
/// directory. The walk stops there, like the compiler's syntax test driver, so
/// a group can keep the files its cases import in a subdirectory of its own.
fn collect_slint_files(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let Ok(groups) = std::fs::read_dir(dir) else {
        return results;
    };
    for group in groups.flatten() {
        let group = group.path();
        if !group.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&group) else {
            continue;
        };
        results.extend(
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|e| e == "slint")),
        );
    }
    results.sort();
    results
}
