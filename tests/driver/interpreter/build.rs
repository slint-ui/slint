// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");

    let mut tests_file = std::fs::File::create(&tests_file_path)?;

    for testcase in test_driver_lib::collect_test_cases("cases")?.into_iter() {
        let test_function_name = testcase.identifier();
        let ignored = testcase.is_ignored("interpreter");

        write!(
            tests_file,
            r##"
            #[test]
            {ignore}
            fn test_interpreter_{function_name}() {{
                interpreter::test(&test_driver_lib::TestCase{{
                    absolute_path: std::path::PathBuf::from(r#"{absolute_path}"#),
                    relative_path: std::path::PathBuf::from(r#"{relative_path}"#),
                    requested_style: {requested_style},
                }}).unwrap();
            }}
        "##,
            ignore = if ignored { "#[ignore]" } else { "" },
            function_name = test_function_name,
            absolute_path = testcase.absolute_path.to_string_lossy(),
            relative_path = testcase.relative_path.to_string_lossy(),
            requested_style =
                testcase.requested_style.map_or("None".into(), |style| format!("Some({style:?})")),
        )?;
    }

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");

    Ok(())
}
