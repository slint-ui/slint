// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::io::Write;
use std::path::PathBuf;

fn os_dylib_prefix_and_suffix() -> (&'static str, &'static str) {
    if cfg!(target_os = "windows") {
        ("", "dll")
    } else if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        ("lib", "dylib")
    } else {
        ("lib", "so")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // target/{debug|release}/build/package/out/ -> target/{debug|release}
    let mut target_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    target_dir.pop();
    target_dir.pop();
    target_dir.pop();

    let nodejs_native_lib_name = {
        let (prefix, suffix) = os_dylib_prefix_and_suffix();
        format!("{}slint_node_native.{}", prefix, suffix)
    };
    println!(
        "cargo:rustc-env=SLINT_NODE_NATIVE_LIB={}",
        target_dir.join(nodejs_native_lib_name).display()
    );

    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");

    let mut tests_file = std::fs::File::create(&tests_file_path)?;

    for testcase in test_driver_lib::collect_test_cases("cases")? {
        println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
        let test_function_name = testcase.identifier();

        write!(
            tests_file,
            r##"
            #[test]
            fn test_nodejs_{function_name}() {{
                nodejs::test(&test_driver_lib::TestCase{{
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
