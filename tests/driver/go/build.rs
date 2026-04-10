// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::{BufWriter, Write};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let output_dir = std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).to_path_buf();
    let target_dir = output_dir
        .ancestors()
        .nth(3)
        .unwrap_or_else(|| {
            panic!("failed to locate target dir for {}", Path::new(&manifest_dir).display())
        })
        .to_path_buf();
    println!("cargo:rustc-env=CPP_LIB_PATH={}/deps", target_dir.display());

    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");
    let mut tests_file = BufWriter::new(std::fs::File::create(&tests_file_path)?);

    for testcase in test_driver_lib::collect_test_cases("cases")?
        .into_iter()
        .filter(|testcase| testcase.requested_style.is_none())
    {
        println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
        let test_function_name = testcase.identifier();
        let ignored = testcase.is_ignored("go");

        write!(
            tests_file,
            r##"
            #[test]
            {ignore}
            fn test_go_{function_name}() {{
                godriver::test(&test_driver_lib::TestCase{{
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

    tests_file.flush()?;

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");

    Ok(())
}
