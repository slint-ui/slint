// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A test harness that runs every test in its own process, on that process's main thread.
//!
//! libtest runs each test on a worker thread, but Qt widgets and AppKit controls only work
//! on the process main thread.
//! A test binary with `harness = false` registers its tests with `#[satchel::test]`
//! and calls [`test_main`] from its `main`.
//! The parent process forks one child per test with `--exact <name>`,
//! the convention cargo nextest uses, and the child runs that single test on its main thread.

// cSpell: ignore nextest

use libtest_mimic::{Arguments, Failed, Trial};
use satchel::TestCase;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long a test's process may run before it gets killed and the test fails.
const TEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Runs `tests`, forking a process per test, and exits with the result.
/// The child calls `init` before its test.
pub fn test_main(tests: impl Iterator<Item = &'static TestCase>, init: fn()) {
    let mut args = Arguments::from_args();
    // One test at a time unless `--test-threads` says otherwise: the tests' windows open at the
    // same spot, and on macOS a window covered by another test's window isn't rendered.
    args.test_threads.get_or_insert(1);
    let tests: Vec<_> = tests.collect();

    // cargo forwards `--exact <name>` to every test binary of a package, so a name
    // that belongs to another binary falls through and reports zero matching tests.
    if args.exact && !args.list {
        let single = args
            .filter
            .as_deref()
            .and_then(|name| tests.iter().find(|test| qualified_name(test) == name));
        if let Some(test) = single {
            init();
            (test.test_fn)();
            return;
        }
    }

    let trials = tests
        .into_iter()
        .map(|test| {
            let name = qualified_name(test);
            Trial::test(name.clone(), move || run_forked(&name))
                .with_ignored_flag(test.ignore.is_some())
        })
        .collect();
    libtest_mimic::run(&args, trials).exit();
}

fn qualified_name(test: &TestCase) -> String {
    format!("{}::{}", test.module_path, test.name)
}

fn run_forked(name: &str) -> Result<(), Failed> {
    println!("### FORKING TEST: {name}");
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--exact", name])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() > TEST_TIMEOUT {
            child.kill()?;
            child.wait()?;
            println!("### SUBPROCESS TIMED OUT");
            return Err(Failed::from(format!(
                "Test {name} timed out after {} seconds",
                TEST_TIMEOUT.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    println!("### SUBPROCESS STATUS: {status}");

    if status.success() {
        Ok(())
    } else {
        Err(Failed::from(format!("Test {name} failed in subprocess")))
    }
}
