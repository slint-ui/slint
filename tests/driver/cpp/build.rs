// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// The root dir of the git repository
fn root_dir() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // $root/tests/driver/driver/ -> $root
    root.pop();
    root.pop();
    root.pop();
    root
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Variables that cc.rs needs.
    println!("cargo:rustc-env=TARGET={}", std::env::var("TARGET").unwrap());
    println!("cargo:rustc-env=HOST={}", std::env::var("HOST").unwrap());
    println!("cargo:rustc-env=OPT_LEVEL={}", std::env::var("OPT_LEVEL").unwrap());

    // target/{debug|release}/build/package/out/ -> target/{debug|release}
    let mut target_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    target_dir.pop();
    target_dir.pop();
    target_dir.pop();

    println!("cargo:rustc-env=CPP_LIB_PATH={}/deps", target_dir.display());

    let generated_include_dir = std::env::var_os("DEP_SLINT_CPP_GENERATED_INCLUDE_DIR")
        .expect("the slint-cpp crate needs to provide the meta-data that points to the directory with the generated includes");
    println!(
        "cargo:rustc-env=GENERATED_CPP_HEADERS_PATH={}",
        Path::new(&generated_include_dir).display()
    );
    let root_dir = root_dir();
    println!("cargo:rustc-env=CPP_API_HEADERS_PATH={}/api/cpp/include", root_dir.display());

    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");

    let mut tests_file = BufWriter::new(std::fs::File::create(&tests_file_path)?);

    for testcase in test_driver_lib::collect_test_cases("cases")? {
        let test_function_name = testcase.identifier();
        let ignored = testcase.is_ignored("cpp");

        write!(
            tests_file,
            r##"
            #[test]
            {ignore}
            fn test_cpp_{function_name}() {{
                cppdriver::test(&test_driver_lib::TestCase{{
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

    tests_file.flush()?;

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
    Ok(())
}
