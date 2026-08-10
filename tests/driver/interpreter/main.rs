// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore vlayout
#[cfg(test)]
mod interpreter;

include!(env!("TEST_FUNCTIONS"));

// Run an example .slint file (path relative to the repo root) through the interpreter.
// The list of examples and the SLINT_TEST_FILTER handling live in build.rs.
#[cfg(test)]
#[allow(dead_code)] // unused when SLINT_TEST_FILTER excludes every example
fn run_example(path: &str) {
    let relative_path = std::path::PathBuf::from(format!("../../../{path}"));
    let absolute_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&relative_path);
    interpreter::test(&test_driver_lib::TestCase {
        absolute_path,
        relative_path,
        requested_style: None,
    })
    .unwrap();
}

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
