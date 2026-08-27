// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Integration tests for the `slint-viewer` binary.

use std::io::Write;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_slint-viewer");

/// Exit code returned by the plain run path on compile failure. `exit(-1)`
/// surfaces as `-1` on Windows and as `255` (the low byte) on Unix.
const COMPILE_ERROR_EXIT: i32 = if cfg!(windows) { -1 } else { 255 };

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN).args(args).output().expect("failed to spawn slint-viewer");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn write_slint(content: &str) -> tempfile::NamedTempFile {
    let mut f =
        tempfile::Builder::new().suffix(".slint").tempfile().expect("creating temp .slint file");
    f.write_all(content.as_bytes()).expect("writing temp .slint file");
    f
}

// --- Argument parsing / CLI validation ----------------------------------

#[test]
fn unknown_argument_is_rejected() {
    let (code, _stdout, stderr) = run(&["--definitely-not-a-real-flag"]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("unexpected argument") || stderr.contains("unrecognized"),
        "stderr was:\n{stderr}"
    );
}

#[test]
fn missing_value_for_screenshot() {
    let (code, _stdout, stderr) = run(&["--screenshot"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("a value is required"), "stderr was:\n{stderr}");
}

#[test]
fn missing_path_argument() {
    // Without `--remote`, a path is required.
    let (code, _stdout, stderr) = run(&[]);
    assert_eq!(code, 2);
    assert!(stderr.contains("required") || stderr.contains("Usage"), "stderr was:\n{stderr}");
}

#[test]
fn screenshot_and_auto_reload_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.png");
    let (code, _stdout, stderr) =
        run(&["--screenshot", out.to_str().unwrap(), "--auto-reload", "x.slint"]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("Cannot pass both --auto-reload and --screenshot"),
        "stderr was:\n{stderr}"
    );
    assert!(!out.exists(), "screenshot file should not have been written");
}

#[test]
fn screenshot_and_save_data_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.png");
    let (code, _stdout, stderr) =
        run(&["--screenshot", out.to_str().unwrap(), "--save-data", "x.json", "x.slint"]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("Cannot pass both --save-data and --screenshot"),
        "stderr was:\n{stderr}"
    );
    assert!(!out.exists(), "screenshot file should not have been written");
}

#[cfg(feature = "remote")]
#[test]
fn screenshot_and_remote_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.png");
    let (code, _stdout, stderr) = run(&["--screenshot", out.to_str().unwrap(), "--remote"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("Cannot pass both --remote and --screenshot"), "stderr was:\n{stderr}");
    assert!(!out.exists(), "screenshot file should not have been written");
}

#[test]
fn auto_reload_and_save_data_conflict() {
    let (code, _stdout, stderr) = run(&["--auto-reload", "--save-data", "x.json", "x.slint"]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("Cannot pass both --auto-reload and --save-data"),
        "stderr was:\n{stderr}"
    );
}

// --- Errors loading the .slint file ------------------------------------

#[test]
fn nonexistent_file_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("does_not_exist.slint");
    let (code, _stdout, stderr) = run(&[missing.to_str().unwrap()]);
    assert_eq!(code, COMPILE_ERROR_EXIT);
    assert!(
        stderr.contains("Could not load") || stderr.contains("No such file"),
        "stderr was:\n{stderr}"
    );
}

#[test]
fn syntax_error_is_reported() {
    let f = write_slint("garbage not valid syntax\n");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.png");
    let (code, _stdout, stderr) =
        run(&["--screenshot", out.to_str().unwrap(), f.path().to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stderr.contains("Parse error"), "stderr was:\n{stderr}");
    assert!(!out.exists(), "screenshot file should not have been written");
}

#[test]
fn file_with_no_component_is_reported() {
    let f = write_slint("// nothing in here\n");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.png");
    let (code, _stdout, stderr) =
        run(&["--screenshot", out.to_str().unwrap(), f.path().to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stderr.contains("No component found"), "stderr was:\n{stderr}");
    assert!(!out.exists(), "screenshot file should not have been written");
}

// --- SystemTrayIcon components ------------------------------------------

// The last export is the tray, so it is what the viewer picks by default.
const TRAY_AND_WINDOW: &str = r#"
export component MainWindow inherits Window {
    Rectangle { background: green; }
}
export component Tray inherits SystemTrayIcon {
    tooltip: "tray";
}
"#;

#[test]
fn tray_component_is_rejected() {
    let f = write_slint(TRAY_AND_WINDOW);
    let (code, _stdout, stderr) = run(&[f.path().to_str().unwrap()]);
    assert_eq!(code, COMPILE_ERROR_EXIT);
    assert!(
        stderr.contains("'Tray' is a SystemTrayIcon, which the viewer cannot display"),
        "stderr was:\n{stderr}"
    );
}

#[test]
fn tray_component_is_rejected_in_screenshot_mode() {
    let f = write_slint(TRAY_AND_WINDOW);
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.png");
    let (code, _stdout, stderr) =
        run(&["--screenshot", out.to_str().unwrap(), f.path().to_str().unwrap()]);
    assert_eq!(code, COMPILE_ERROR_EXIT);
    assert!(stderr.contains("SystemTrayIcon"), "stderr was:\n{stderr}");
    assert!(!out.exists(), "screenshot file should not have been written");
}

#[test]
fn component_flag_selects_the_window() {
    let f = write_slint(TRAY_AND_WINDOW);
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.png");
    let (code, _stdout, stderr) = run(&[
        "--component",
        "MainWindow",
        "--screenshot",
        out.to_str().unwrap(),
        "--size",
        "64x64",
        f.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stderr was:\n{stderr}");
    assert!(out.exists(), "screenshot file should have been written");
}

// --- Check mode --------------------------------------------------------

#[test]
fn check_valid_file_exits_zero() {
    let f = write_slint("export component Ok { Text { text: \"hi\"; } }\n");
    let (code, _stdout, stderr) = run(&["--check", f.path().to_str().unwrap()]);
    assert_eq!(code, 0, "stderr was:\n{stderr}");
}

#[test]
fn check_syntax_error_exits_one() {
    let f = write_slint("export component Bad { Text { letter-spacing: 1em; } }\n");
    let (code, _stdout, stderr) = run(&["--check", f.path().to_str().unwrap()]);
    assert_eq!(code, 1, "stderr was:\n{stderr}");
    assert!(stderr.contains("Invalid unit 'em'"), "stderr was:\n{stderr}");
}

#[test]
fn check_conflicts_with_auto_reload() {
    let f = write_slint("export component Ok { }\n");
    let (code, _stdout, stderr) = run(&["--check", "--auto-reload", f.path().to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("--check") && stderr.contains("--auto-reload"),
        "stderr was:\n{stderr}"
    );
}

#[test]
fn check_conflicts_with_screenshot() {
    let f = write_slint("export component Ok { }\n");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.png");
    let (code, _stdout, stderr) =
        run(&["--check", "--screenshot", out.to_str().unwrap(), f.path().to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stderr.contains("--check") && stderr.contains("--screenshot"), "stderr was:\n{stderr}");
}

// --- Screenshot rendering ----------------------------------------------

// Cases known to produce the same RGB output as their reference under both
// the software renderer and Skia's software rasterizer. To add a case, drop
// `<name>.slint` into tests/screenshots/cases/basic/, generate the references
// via `SLINT_CREATE_SCREENSHOTS=1 cargo test -p test-driver-screenshots`,
// then verify that the viewer's `--screenshot` output matches both.
const SCREENSHOT_CASES: &[&str] = &["rgb", "linear-gradients", "radial-gradients"];

#[cfg(any(
    feature = "renderer-skia",
    feature = "renderer-skia-opengl",
    feature = "renderer-skia-vulkan"
))]
const REFERENCE_RENDERER: &str = "skia";
#[cfg(not(any(
    feature = "renderer-skia",
    feature = "renderer-skia-opengl",
    feature = "renderer-skia-vulkan"
)))]
const REFERENCE_RENDERER: &str = "software";

#[test]
fn screenshot_matches_reference() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases_dir = manifest.join("../../tests/screenshots/cases/basic");
    let references_dir =
        manifest.join("../../tests/screenshots/references").join(REFERENCE_RENDERER).join("basic");

    let tmp = tempfile::tempdir().unwrap();
    let mut failures = vec![];

    for name in SCREENSHOT_CASES {
        let input = cases_dir.join(format!("{name}.slint"));
        let reference = references_dir.join(format!("{name}.png"));
        let output = tmp.path().join(format!("{name}.png"));

        if !reference.exists() {
            failures.push(format!("[{name}] missing reference {}", reference.display()));
            continue;
        }

        let (code, _stdout, stderr) =
            run(&["--screenshot", output.to_str().unwrap(), input.to_str().unwrap()]);
        if code != 0 {
            failures.push(format!("[{name}] viewer exited with {code}; stderr:\n{stderr}"));
            continue;
        }

        if let Err(e) = compare_rgb(&output, &reference) {
            failures.push(format!("[{name}] {e}"));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn compare_rgb(actual: &Path, reference: &Path) -> Result<(), String> {
    // The reference images in tests/screenshots/references/ are written without
    // an alpha channel, so we compare only the RGB channels here.
    let a = image::open(actual)
        .map_err(|e| format!("loading actual {}: {e}", actual.display()))?
        .to_rgb8();
    let b = image::open(reference)
        .map_err(|e| format!("loading reference {}: {e}", reference.display()))?
        .to_rgb8();
    if a.dimensions() != b.dimensions() {
        return Err(format!(
            "size mismatch: actual {:?} vs reference {:?}",
            a.dimensions(),
            b.dimensions()
        ));
    }
    let mut max_diff: u8 = 0;
    let mut diff_pixels: usize = 0;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        if pa != pb {
            diff_pixels += 1;
            for (ca, cb) in pa.0.iter().zip(pb.0.iter()) {
                max_diff = max_diff.max(ca.abs_diff(*cb));
            }
        }
    }
    // A tiny tolerance soaks up subpixel rounding differences across platforms.
    if max_diff > 4 {
        return Err(format!(
            "pixels differ from reference: {diff_pixels} pixels, max channel diff {max_diff}"
        ));
    }
    Ok(())
}

// --- Screenshot size ---------------------------------------------------

#[test]
fn size_requires_screenshot() {
    let f = write_slint("export component App { }\n");
    let (code, _stdout, stderr) = run(&["--size", "100x50", f.path().to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stderr.contains("--screenshot"), "stderr was:\n{stderr}");
}

#[test]
fn screenshot_size_sets_dimensions() {
    let f = write_slint("export component App { Rectangle { background: red; } }\n");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.png");
    let (code, _stdout, stderr) = run(&[
        "--screenshot",
        out.to_str().unwrap(),
        "--size",
        "120x80",
        f.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stderr was:\n{stderr}");
    assert_eq!(image::open(&out).expect("opening screenshot").to_rgb8().dimensions(), (120, 80));
}

#[test]
fn invalid_size_is_reported() {
    let f = write_slint("export component App { }\n");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.png");
    let (code, _stdout, stderr) = run(&[
        "--screenshot",
        out.to_str().unwrap(),
        "--size",
        "not-a-size",
        f.path().to_str().unwrap(),
    ]);
    assert_ne!(code, 0);
    assert!(stderr.contains("Invalid --size"), "stderr was:\n{stderr}");
    assert!(!out.exists(), "screenshot file should not have been written");
}
