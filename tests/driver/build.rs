use std::io::Write;
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    // Variables that cc.rs needs.
    println!("cargo:rustc-env=TARGET={}", std::env::var("TARGET").unwrap());
    println!("cargo:rustc-env=HOST={}", std::env::var("HOST").unwrap());
    println!("cargo:rustc-env=OPT_LEVEL={}", std::env::var("OPT_LEVEL").unwrap());

    // Variables that we need.
    println!("cargo:rustc-env=CARGO={}", std::env::var("CARGO").unwrap());

    let mut generated_include_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    generated_include_dir.pop();
    generated_include_dir.pop();
    generated_include_dir.pop();

    let lib_dir = generated_include_dir.clone();
    println!("cargo:rustc-env=CPP_LIB_PATH={}", lib_dir.display());

    generated_include_dir.push("include");
    println!("cargo:rustc-env=GENERATED_CPP_HEADERS_PATH={}", generated_include_dir.display());

    let mut api_includes = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    api_includes.pop();
    api_includes.pop();
    api_includes = api_includes.join("api/sixtyfps-cpp/include");

    println!("cargo:rustc-env=CPP_API_HEADERS_PATH={}", api_includes.display());

    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");

    let mut tests_file = std::fs::File::create(&tests_file_path)?;

    let mut test_dirs = std::collections::HashSet::new();

    for testcase in test_driver_lib::collect_test_cases()? {
        println!("cargo:rerun-if-changed={}", testcase.absolute_path.to_string_lossy());

        test_dirs.insert({
            let mut dir = testcase.absolute_path.clone();
            dir.pop();
            dir
        });

        let test_function_name =
            testcase.relative_path.with_extension("").to_string_lossy().replace("/", "_");

        write!(
            tests_file,
            r#"
            #[test]
            fn test_cpp_{function_name}() {{
                cpp::test(&test_driver_lib::TestCase{{
                    absolute_path: std::path::PathBuf::from("{absolute_path}"),
                    relative_path: std::path::PathBuf::from("{relative_path}"),
                }}).unwrap();
            }}
        "#,
            function_name = test_function_name,
            absolute_path = testcase.absolute_path.to_string_lossy(),
            relative_path = testcase.relative_path.to_string_lossy(),
        )?;
    }

    test_dirs.iter().for_each(|dir| {
        println!("cargo:rerun-if-changed={}", dir.to_string_lossy());
    });

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());

    Ok(())
}
