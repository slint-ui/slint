// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");

    let mut tests_file = std::fs::File::create(&tests_file_path)?;

    for testcase in test_driver_lib::collect_test_cases("cases")?.into_iter().filter(|testcase| {
        // Style testing not supported yet
        testcase.requested_style.is_none()
    }) {
        println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
        let test_function_name = testcase.identifier();
        let source = std::fs::read_to_string(&testcase.absolute_path)?;
        // A `js` ignore also applies here: both drivers run the same js blocks
        // against the same JS API. Cases with include or library paths are not
        // supported: the wasm compiler has no include-path configuration.
        let ignored = testcase.is_ignored("browser")
            || testcase.is_ignored("js")
            || test_driver_lib::extract_include_paths(&source).next().is_some()
            || test_driver_lib::extract_library_paths(&source).next().is_some();

        write!(
            tests_file,
            r##"
            #[test]
            {ignore}
            fn test_browser_{function_name}() {{
                browser::test(&test_driver_lib::TestCase{{
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
    }

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());

    Ok(())
}
