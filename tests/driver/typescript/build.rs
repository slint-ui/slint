// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1",);

    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");

    let mut tests_file = std::fs::File::create(&tests_file_path)?;

    let mut typecheck_paths = Vec::new();

    for testcase in test_driver_lib::collect_test_cases("cases")?.into_iter().filter(|testcase| {
        // Style testing not supported yet
        testcase.requested_style.is_none()
    }) {
        println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
        let test_function_name = testcase.identifier();
        let ignored = testcase.is_ignored("ts");

        write!(
            tests_file,
            r##"
            #[test]
            {ignore}
            fn test_typescript_{function_name}() {{
                typescript::test(&test_driver_lib::TestCase{{
                    absolute_path: std::path::PathBuf::from(r#"{absolute_path}"#),
                    relative_path: std::path::PathBuf::from(r#"{relative_path}"#),
                    requested_style: None,
                }}).unwrap();
            }}
        "##,
            ignore = if ignored { "#[ignore]" } else { "" },
            function_name = test_function_name,
            absolute_path = testcase.absolute_path.to_string_lossy(),
            relative_path = testcase.relative_path.to_string_lossy(),
        )?;

        if !ignored {
            typecheck_paths.push(testcase.absolute_path.to_string_lossy().to_string());
        }
    }

    // Generate a single test that runs tsc on all generated .d.ts files at once
    writeln!(tests_file, "\n    #[test]\n    fn typecheck_all_generated_declarations() {{")?;
    writeln!(tests_file, "        typescript::typecheck_all(&[")?;
    for path in &typecheck_paths {
        writeln!(tests_file, "            r#\"{path}\"#,")?;
    }
    writeln!(tests_file, "        ]);")?;
    writeln!(tests_file, "    }}")?;

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());

    Ok(())
}
