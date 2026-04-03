// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::error::Error;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::{fs::File, io::Write};

/// Build the Swift package once and return its absolute path.
static SWIFT_PACKAGE_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let swift_dir =
        std::fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../api/swift"))
            .expect("Could not canonicalize api/swift path");

    // Build the Rust static library with interpreter support
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let o = std::process::Command::new(cargo)
        .args(["build", "--lib", "-p", "slint-swift", "--features", "interpreter"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("Could not launch cargo build for slint-swift");

    if !o.status.success() {
        eprintln!(
            "STDERR:\n{}\nSTDOUT:\n{}",
            String::from_utf8_lossy(&o.stderr),
            String::from_utf8_lossy(&o.stdout),
        );
        panic!("cargo build --lib -p slint-swift failed: {:?}", o.status);
    }

    // Build the Swift package
    let o = std::process::Command::new("swift")
        .args(["build"])
        .current_dir(&swift_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("Could not launch swift build");

    if !o.status.success() {
        eprintln!(
            "STDERR:\n{}\nSTDOUT:\n{}",
            String::from_utf8_lossy(&o.stderr),
            String::from_utf8_lossy(&o.stdout),
        );
        panic!("swift build failed: {:?}", o.status);
    }

    swift_dir
});

#[allow(dead_code)] // Called from generated #[test] functions in build.rs
pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;

    let swift_blocks: Vec<_> = test_driver_lib::extract_test_functions(&source)
        .filter(|x| x.language_id == "swift")
        .collect();

    // If there are no Swift test blocks, skip
    if swift_blocks.is_empty() {
        return Ok(());
    }

    // Ensure the Swift package is built
    let swift_package_dir = SWIFT_PACKAGE_DIR.clone();

    let dir = tempfile::tempdir()?;

    // Create a temporary SPM executable package that depends on the Slint package
    let sources_dir = dir.path().join("Sources");
    std::fs::create_dir_all(&sources_dir)?;

    // Write Package.swift
    let mut pkg = File::create(dir.path().join("Package.swift"))?;
    write!(
        pkg,
        r#"// swift-tools-version: 6.2
import PackageDescription
let package = Package(
    name: "SlintTestRunner",
    platforms: [.macOS(.v13), .iOS(.v16)],
    dependencies: [
        .package(name: "Slint", path: "{slint_pkg}"),
    ],
    targets: [
        .executableTarget(
            name: "SlintTestRunner",
            dependencies: [
                .product(name: "Slint", package: "Slint"),
                .product(name: "SlintInterpreter", package: "Slint"),
            ],
            path: "Sources"
        ),
    ]
)
"#,
        slint_pkg = swift_package_dir.to_string_lossy(),
    )?;
    drop(pkg);

    // Write the test source file
    let test_file = sources_dir.join("main.swift");
    let mut f = File::create(&test_file)?;

    write!(
        f,
        r#"// Auto-generated Swift test file
import Foundation
import Slint
import SlintInterpreter

// Assertion helpers
func assertEqual<T: Equatable>(_ a: T, _ b: T, file: String = #file, line: Int = #line) {{
    if a != b {{
        print("FAIL: \(file):\(line): \(a) != \(b)")
        exit(1)
    }}
}}

func assertClose(_ a: Double, _ b: Double, tolerance: Double = 0.001,
                 file: String = #file, line: Int = #line) {{
    if abs(a - b) > tolerance {{
        print("FAIL: \(file):\(line): \(a) not close to \(b)")
        exit(1)
    }}
}}

func assertTrue(_ condition: Bool, file: String = #file, line: Int = #line) {{
    if !condition {{
        print("FAIL: \(file):\(line): condition is false")
        exit(1)
    }}
}}

func assertFalse(_ condition: Bool, file: String = #file, line: Int = #line) {{
    if condition {{
        print("FAIL: \(file):\(line): condition is true")
        exit(1)
    }}
}}

// Load the .slint file via the interpreter
let compiler = SlintCompiler()
let definition = compiler.buildFromSource(
    try! String(contentsOfFile: {slint_path:?}, encoding: .utf8),
    path: {slint_path:?}
)!
let instance = definition.createInstance()!

"#,
        slint_path = testcase.absolute_path.to_string_lossy(),
    )?;

    // Write each Swift test block wrapped in `do { }` to avoid trailing closure ambiguity
    for block in &swift_blocks {
        writeln!(f, "do {{\n    {}\n}}\n", block.source.replace('\n', "\n    "))?;
    }

    // Write the success exit
    writeln!(f, "print(\"PASS\")")?;
    writeln!(f, "exit(0)")?;

    drop(f);

    // Find the Rust target directory for the static library
    let target_debug_dir = swift_package_dir.join("../../target/debug");
    let target_debug_dir =
        std::fs::canonicalize(&target_debug_dir).unwrap_or(target_debug_dir.clone());

    // Build and run the temporary package
    let output = std::process::Command::new("swift")
        .args([
            "run",
            "--package-path",
            dir.path().to_str().unwrap(),
            "-Xlinker",
            &format!("-L{}", target_debug_dir.to_string_lossy()),
        ])
        .env("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not launch swift run: {err}"))?;

    if !output.status.success() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
        print!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(format!(
            "Swift test failed for {}:\nSTDOUT: {}\nSTDERR: {}",
            testcase.relative_path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )
        .into());
    }

    Ok(())
}
