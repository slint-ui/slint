// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use libtest_mimic::{Arguments, Failed};
use satchel::TestCase;
use std::process::{Command, Stdio};

fn extract_single_test(args: &Arguments) -> Option<String> {
    // Note: Cargo nextest also uses `--exact` to run a single test,
    // so we can use that to detect if we're already in a subprocess.
    if args.exact { args.filter.clone() } else { None }
}

pub(crate) fn run_forked(name: &str) -> Result<(), Failed> {
    println!("### FORKING TEST: {name}");
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    println!("### SUBPROCESS STATUS: {}", status);

    if !status.success() {
        Err(Failed::from(format!("Test {name} failed in subprocess")))
    } else {
        Ok(())
    }
}

// single-test mode: run only that test on the main process, and don't fork a subprocess.
pub fn run_test(test_name: &str) {
    let tests: Vec<_> = satchel::get_tests!().collect();

    let test = tests
        .iter()
        .find(|test| qualified_name(test) == test_name)
        .unwrap_or_else(|| panic!("Test {test_name} not found"));
    crate::init();
    (test.test_fn)();
}

fn qualified_name(test: &TestCase) -> String {
    format!("{}::{}", test.module_path, test.name)
}

// Run all tests, but fork a subprocess for each test.
pub fn fork_tests(args: Arguments) {
    let tests = satchel::get_tests!()
        .map(|test| {
            let name = qualified_name(&test);
            let test_fn = {
                let name = name.clone();
                move || run_forked(&name)
            };

            libtest_mimic::Trial::test(name, test_fn)
        })
        .collect();
    libtest_mimic::run(&args, tests).exit();
}

pub fn test_main() {
    let args = libtest_mimic::Arguments::from_args();

    if let Some(test_filter) = extract_single_test(&args) {
        run_test(&test_filter);
    } else {
        fork_tests(args);
    }
}
