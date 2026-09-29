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

    // Cargo doesn't tell a build script where a dependency's artifacts end up,
    // so the driver asks Cargo for slint-cpp's cdylib at run time. It repeats
    // this build's features, profile, and target so that the query is a no-op.
    let features = std::env::vars()
        .filter_map(|(var, _)| {
            var.strip_prefix("CARGO_FEATURE_").map(|f| f.to_lowercase().replace('_', "-"))
        })
        .collect::<Vec<_>>();
    let mut cargo_args =
        vec!["--no-default-features".to_string(), format!("--features={}", features.join(","))];
    if std::env::var("PROFILE").unwrap() == "release" {
        cargo_args.push("--release".into());
    }
    // An explicit `--target` puts the output in a per-target directory.
    let target = std::env::var("TARGET").unwrap();
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    if out_dir.components().any(|c| c.as_os_str() == target.as_str()) {
        cargo_args.push(format!("--target={target}"));
    }
    println!("cargo:rustc-env=CPP_LIB_CARGO_ARGS={}", cargo_args.join(" "));
    println!(
        "cargo:rustc-env=SLINT_CPP_OUT_DIR={}",
        std::env::var("DEP_SLINT_CPP_OUT_DIR").unwrap()
    );

    let generated_include_dir = std::env::var_os("DEP_SLINT_CPP_GENERATED_INCLUDE_DIR")
        .expect("the slint-cpp crate needs to provide the meta-data that points to the directory with the generated includes");
    println!(
        "cargo:rustc-env=GENERATED_CPP_HEADERS_PATH={}",
        Path::new(&generated_include_dir).display()
    );
    let root_dir = root_dir();
    println!("cargo:rustc-env=CPP_API_HEADERS_PATH={}/api/cpp/include", root_dir.display());

    let tests_file_path = out_dir.join("test_functions.rs");

    let mut tests_file = BufWriter::new(std::fs::File::create(&tests_file_path)?);

    let live_preview = std::env::var("SLINT_LIVE_PREVIEW").is_ok();
    println!("cargo::rerun-if-env-changed=SLINT_LIVE_PREVIEW");

    for testcase in test_driver_lib::collect_test_cases("cases")? {
        let test_function_name = testcase.identifier();
        let ignored = if testcase.is_ignored("cpp") {
            "#[ignore = \"testcase ignored for cpp\"]"
        } else if live_preview
            && (testcase.is_ignored("live-preview") || testcase.is_ignored("cpp-live-preview"))
        {
            "#[ignore = \"testcase ignored in live-preview mode\"]"
        } else {
            ""
        };

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
            ignore = ignored,
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
