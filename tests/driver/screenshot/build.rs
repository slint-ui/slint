// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::io::Write;
use std::path::Path;

/// Returns a list of all the `.slint` files in the `tests/cases` subfolders.
pub fn collect_test_cases() -> std::io::Result<Vec<test_driver_lib::TestCase>> {
    let mut results = vec![];

    let case_root_dir: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "..", "..", "screenshot"].iter().collect();

    println!("cargo:rerun-if-env-changed=SLINT_TEST_FILTER");
    let filter = std::env::var("SLINT_TEST_FILTER").ok();

    for entry in walkdir::WalkDir::new(case_root_dir.clone()).follow_links(true) {
        let entry = entry?;
        let absolute_path = entry.into_path();
        if absolute_path.is_dir() {
            println!("cargo:rerun-if-changed={}", absolute_path.display());
            continue;
        }
        let relative_path =
            std::path::PathBuf::from(absolute_path.strip_prefix(&case_root_dir).unwrap());
        if let Some(filter) = &filter {
            if !relative_path.to_str().unwrap().contains(filter) {
                continue;
            }
        }
        if let Some(ext) = absolute_path.extension() {
            if ext == "60" || ext == "slint" {
                results.push(test_driver_lib::TestCase { absolute_path, relative_path });
            }
        }
    }
    Ok(results)
}


fn main() -> std::io::Result<()> {
    let mut generated_file = std::fs::File::create(
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("generated.rs"),
    )?;

    for testcase in collect_test_cases()? {
        println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
        let mut module_name = testcase.identifier();
        if module_name.starts_with(|c: char| !c.is_ascii_alphabetic()) {
            module_name.insert(0, '_');
        }
        writeln!(generated_file, "#[path=\"{0}.rs\"] mod r#{0};", module_name)?;
        let source = std::fs::read_to_string(&testcase.absolute_path)?;

        let mut output = std::fs::File::create(
            Path::new(&std::env::var_os("OUT_DIR").unwrap()).join(format!("{}.rs", module_name)),
        )?;

        #[cfg(not(feature = "build-time"))]
        if !generate_macro(&source, &mut output, testcase)? {
            continue;
        }
        #[cfg(feature = "build-time")]
        generate_source(&source, &mut output, testcase)?;

       
        for (i, x) in test_driver_lib::extract_test_functions(&source)
            .filter(|x| x.language_id == "rust")
            .enumerate()
        {
            write!(
                output,
                r"
#[test] fn t_{}() -> Result<(), Box<dyn std::error::Error>> {{
    use i_slint_backend_testing as slint_testing;
    {}
    Ok(())
}}",
                i,
                x.source.replace('\n', "\n    ")
            )?;
        }
    }

    //Make sure to use a consistent style
    println!("cargo:rustc-env=SLINT_STYLE=fluent");

    println!("cargo:rustc-env=SLINT_EXPERIMENTAL_SYNTAX=true");

    Ok(())
}

#[cfg(not(feature = "build-time"))]
fn generate_macro(
    source: &str,
    output: &mut std::fs::File,
    testcase: test_driver_lib::TestCase,
) -> Result<bool, std::io::Error> {
    if source.contains("\\{") {
        // Unfortunately, \{ is not valid in a rust string so it cannot be used in a slint! macro
        output.write_all(b"#[test] #[ignore] fn ignored_because_string_template() {{}}")?;
        return Ok(false);
    }
    let include_paths = test_driver_lib::extract_include_paths(source);
    output.write_all(b"slint::slint!{")?;
    for path in include_paths {
        let mut abs_path = testcase.absolute_path.clone();
        abs_path.pop();
        abs_path.push(path);

        output.write_all(b"#[include_path=r#\"")?;
        output.write_all(abs_path.to_string_lossy().as_bytes())?;
        output.write_all(b"\"#]\n")?;

        println!("cargo:rerun-if-changed={}", abs_path.to_string_lossy());
    }
    let mut abs_path = testcase.absolute_path;
    abs_path.pop();
    output.write_all(b"#[include_path=r#\"")?;
    output.write_all(abs_path.to_string_lossy().as_bytes())?;
    output.write_all(b"\"#]\n")?;
    output.write_all(source.as_bytes())?;
    output.write_all(b"}\n")?;
    Ok(true)
}

#[cfg(feature = "build-time")]
fn generate_source(
    source: &str,
    output: &mut std::fs::File,
    testcase: test_driver_lib::TestCase,
) -> Result<(), std::io::Error> {
    use i_slint_compiler::{diagnostics::BuildDiagnostics, *};

    let include_paths = test_driver_lib::extract_include_paths(source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();

    let mut diag = BuildDiagnostics::default();
    diag.enable_experimental = true;
    let syntax_node = parser::parse(source.to_owned(), Some(&testcase.absolute_path), &mut diag);
    let mut compiler_config = CompilerConfiguration::new(generator::OutputFormat::Rust);
    compiler_config.include_paths = include_paths;
    // compiler_config.embed_resources = EmbedResourcesKind::EmbedTextures;
    compiler_config.style = Some("fluent".to_string());
    let (root_component, diag) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));

    if diag.has_error() {
        diag.print_warnings_and_exit_on_error();
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("build error in {:?}", testcase.absolute_path),
        ));
    } else {
        diag.print();
    }

    generator::generate(generator::OutputFormat::Rust, output, &root_component)?;
    Ok(())
}
