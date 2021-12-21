// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");

    let mut tests_file = std::fs::File::create(&tests_file_path)?;

    for testcase in test_driver_lib::collect_test_cases()? {
        let test_function_name = testcase.identifier();

        write!(
            tests_file,
            r##"
            #[test]
            fn test_interpreter_{function_name}() {{
                interpreter::test(&test_driver_lib::TestCase{{
                    absolute_path: std::path::PathBuf::from(r#"{absolute_path}"#),
                    relative_path: std::path::PathBuf::from(r#"{relative_path}"#),
                }}).unwrap();
            }}
        "##,
            function_name = test_function_name,
            absolute_path = testcase.absolute_path.to_string_lossy(),
            relative_path = testcase.relative_path.to_string_lossy(),
        )?;
    }

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());

    Ok(())
}
