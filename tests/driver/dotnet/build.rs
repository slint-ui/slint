// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1",);
    // The cases target the edition of this Slint, like in the other drivers.
    println!(
        "cargo:rustc-env=SLINT_EDITION={}.{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR")
    );

    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let mut tests_file = std::fs::File::create(out_dir.join("test_functions.rs"))?;
    // The cases that the batch in dotnet.rs compiles into one runner.
    let mut cases_file = std::fs::File::create(out_dir.join("cases.txt"))?;

    for testcase in test_driver_lib::collect_test_cases("cases")?.into_iter().filter(|testcase| {
        // Style testing not supported yet
        testcase.requested_style.is_none()
    }) {
        println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
        let ignored = testcase.is_ignored("csharp") || testcase.is_ignored("interpreter");
        let identifier = testcase.identifier();
        let source = std::fs::read_to_string(&testcase.absolute_path)?;
        let edition = test_driver_lib::extract_slint_edition(&source);
        if !ignored {
            writeln!(cases_file, "{identifier}\t{}", testcase.absolute_path.display())?;
        }

        write!(
            tests_file,
            r##"
            #[test]
            {ignore}
            fn test_dotnet_{identifier}() {{
                dotnet::test("{identifier}", {edition:?}).unwrap();
            }}
        "##,
            ignore = if ignored { "#[ignore]" } else { "" },
        )?;
    }

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", out_dir.join("test_functions.rs").display());
    println!("cargo:rustc-env=TEST_CASES={}", out_dir.join("cases.txt").display());
    println!("cargo:rustc-env=RUNNER_DIR={}", out_dir.join("runner").display());

    Ok(())
}
