// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::process::{Command, Stdio};

fn is_single_test_process() -> bool {
    // Note: Cargo nextest also uses `--exact` to run a single test,
    // so we can use that to detect if we're already in a subprocess.
    std::env::args().any(|arg| arg == "--exact")
}

// Run the test binary with `--list` to get the full test name, with module path, etc.
fn full_test_name(name: &str) -> String {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--list"])
        .output()
        .expect("Failed to list test cases!");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in test case list");
    stdout
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .find(|test| test.contains(name))
        .map(String::from)
        .expect(&format!("Unable to find full test name for {name}.\ntests:\n{stdout}"))
}

pub(crate) fn run_forked(name: &str, test: impl FnOnce()) {
    // Ife we're already running a single test process, just run the test directly without forking.
    if is_single_test_process() {
        crate::init();
        test();
        return;
    }

    let name = full_test_name(name);

    println!("### FORKING TEST: {name}");
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to run subprocess");

    println!("### SUBPROCESS STATUS: {}", status);

    if !status.success() {
        panic!("Test {name} failed in subprocess");
    }
}

/// A helper macro that runs the test in a separate process.
/// This is necessary for most platform tests, as they can interfere with
/// each other when running in the same process.
///
/// Note: Using `cargo nextest` will already run every test in a separate process,
/// which is the best way to run these tests.
macro_rules! test {
    {
        fn $name:ident() $body:block
    } => {
        #[test]
        fn $name() {
            crate::cases::harness::run_forked(stringify!($name), || $body);
        }
    };
}
pub(crate) use test;
