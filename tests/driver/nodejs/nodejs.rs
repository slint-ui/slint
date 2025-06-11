// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::error::Error;
use std::sync::LazyLock;
use std::{fs::File, io::Write, path::PathBuf};

#[track_caller]
fn check_output(o: std::process::Output) {
    if !o.status.success() {
        eprintln!(
            "STDERR:\n{}\nSTDOUT:\n{}",
            String::from_utf8_lossy(&o.stderr),
            String::from_utf8_lossy(&o.stdout),
        );
        //panic!("Build Failed {:?}", o.status);
    }
}

static NODE_API_JS_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    let node_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../api/node");

    // On Windows npm is 'npm.cmd', which Rust's process::Command doesn't look for as extension, because
    // it tries to emulate CreateProcess.
    let npm = which::which("npm").unwrap();

    // installs the slint node package dependencies
    let o = std::process::Command::new(npm.clone())
        .arg("install")
        .arg("--no-audit")
        .arg("--ignore-scripts")
        .current_dir(node_dir.clone())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not launch npm install: {err}"))
        .unwrap();

    check_output(o);

    // builds the slint node package in debug
    let o = std::process::Command::new(npm.clone())
        .arg("run")
        .arg("build:testing")
        .current_dir(node_dir.clone())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not launch npm install: {err}"))
        .unwrap();

    check_output(o);

    node_dir.join("dist/index.js")
});

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let slintpath = NODE_API_JS_PATH.clone();

    let dir = tempfile::tempdir()?;

    let mut main_js = File::create(dir.path().join("main.js"))?;
    write!(
        main_js,
        r#"
                const assert = require('assert').strict;
                let slintlib = require(String.raw`{slintpath}`);
                slintlib.private_api.initTesting();
                let slint = slintlib.loadFile(String.raw`{path}`);
        "#,
        slintpath = slintpath.to_string_lossy(),
        path = testcase.absolute_path.to_string_lossy()
    )?;
    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let include_paths = test_driver_lib::extract_include_paths(&source);
    let library_paths = test_driver_lib::extract_library_paths(&source)
        .map(|(k, v)| {
            let mut abs_path = testcase.absolute_path.clone();
            abs_path.pop();
            abs_path.push(v);
            format!("{}={}", k, abs_path.to_string_lossy())
        })
        .collect::<Vec<_>>();
    for x in test_driver_lib::extract_test_functions(&source).filter(|x| x.language_id == "js") {
        write!(main_js, "{{\n    {}\n}}\n", x.source.replace("\n", "\n    "))?;
    }

    let output = std::process::Command::new("node")
        .arg(dir.path().join("main.js"))
        .current_dir(dir.path())
        .env("SLINT_INCLUDE_PATH", std::env::join_paths(include_paths).unwrap())
        .env("SLINT_LIBRARY_PATH", std::env::join_paths(library_paths).unwrap())
        .env("SLINT_SCALE_FACTOR", "1") // We don't have a testing backend, but we can try to force a SF1 as the tests expect.
        .env("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not launch npm start: {err}"))?;

    if !output.status.success() {
        print!("{}", String::from_utf8_lossy(output.stdout.as_ref()));
        print!("{}", String::from_utf8_lossy(output.stderr.as_ref()));
        return Err(String::from_utf8_lossy(output.stderr.as_ref()).into_owned().into());
    }

    Ok(())
}
