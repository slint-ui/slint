// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Runs the test cases in a headless browser: a shared Chromium (driven by the
//! playwright-based runner.mjs) loads a harness page that compiles each case
//! with the wasm interpreter built with the `testing` feature, then evaluates
//! the `test` property and the `js` code blocks. Each `#[test]` sends one
//! request to the runner over a line-based JSON protocol on stdin/stdout.

use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{LazyLock, Mutex};

fn check_output(o: std::process::Output) {
    if !o.status.success() {
        print!("{}", String::from_utf8_lossy(o.stdout.as_ref()));
        print!("{}", String::from_utf8_lossy(o.stderr.as_ref()));
        panic!("Command Failed {:?}", o.status);
    }
}

struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

static SESSION: LazyLock<Mutex<Session>> = LazyLock::new(|| {
    let driver_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = driver_dir.join("../../..").canonicalize().unwrap();

    let pnpm = which::which("pnpm").expect("pnpm must be installed to run the browser tests");

    // Install the runner's dependencies (playwright) and the JS workspace links.
    let o = Command::new(pnpm.clone())
        .arg("install")
        .arg("--config.confirmModulesPurge=false")
        .current_dir(&driver_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Could not launch pnpm install");
    check_output(o);

    // Build the pieces the harness page loads: the shared TypeScript package
    // (ESM build), the browser API, and the wasm module with the testing
    // backend (kept in a separate pkg-testing dir so it never mixes with the
    // regular pkg output).
    for (dir, script) in [
        ("api/js/common", "compile"),
        ("api/js/browser", "compile"),
        ("api/js/browser", "build:wasm:testing"),
    ] {
        let o = Command::new(pnpm.clone())
            .args(["run", script])
            .current_dir(repo_root.join(dir))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap_or_else(|err| panic!("Could not launch pnpm run {script} in {dir}: {err}"));
        check_output(o);
    }

    let node = which::which("node").expect("node must be installed to run the browser tests");
    let mut child = Command::new(node)
        .arg(driver_dir.join("runner.mjs"))
        .arg(&repo_root)
        .current_dir(&driver_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // stderr is inherited: runner/browser diagnostics go to the test output.
        .spawn()
        .expect("Could not launch the browser test runner");

    let stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // The runner prints one "ready" line once the browser loaded the harness.
    let mut line = String::new();
    stdout.read_line(&mut line).expect("failed to read from the browser test runner");
    let ready: serde_json::Value = serde_json::from_str(&line)
        .unwrap_or_else(|_| panic!("unexpected runner output: {line}"));
    assert_eq!(ready["ready"], true, "browser test runner failed to start: {line}");

    Mutex::new(Session { child, stdin, stdout, next_id: 0 })
});

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;

    let js_blocks: Vec<&str> = test_driver_lib::extract_test_functions(&source)
        .filter(|x| x.language_id == "js")
        .map(|x| x.source)
        .collect();

    // Serve the case through the runner's static server, rooted at the
    // repository: imports resolve relative to this URL.
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize()?;
    let base_url = format!(
        "/{}",
        testcase
            .absolute_path
            .canonicalize()?
            .strip_prefix(&repo_root)?
            .to_string_lossy()
            .replace('\\', "/")
    );

    let mut session = SESSION.lock().unwrap_or_else(|poison| poison.into_inner());
    session.next_id += 1;
    let id = session.next_id;

    let request = serde_json::json!({
        "id": id,
        "baseUrl": base_url,
        "source": source,
        "js": js_blocks,
    });

    writeln!(session.stdin, "{request}")?;

    let mut line = String::new();
    loop {
        line.clear();
        if session.stdout.read_line(&mut line)? == 0 {
            return Err("the browser test runner exited unexpectedly".into());
        }
        let response: serde_json::Value = serde_json::from_str(&line)?;
        if response["id"] == id {
            if response["ok"] == true {
                return Ok(());
            }
            let error = response["error"].as_str().unwrap_or("unknown error");
            let console = response["console"]
                .as_array()
                .map(|lines| {
                    lines.iter().filter_map(|l| l.as_str()).collect::<Vec<_>>().join("\n")
                })
                .unwrap_or_default();
            if !console.is_empty() {
                println!("browser console:\n{console}");
            }
            return Err(error.to_owned().into());
        }
    }
}
