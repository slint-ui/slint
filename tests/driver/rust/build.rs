// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::io::{BufWriter, Write};
use std::path::Path;

fn main() -> std::io::Result<()> {
    let mut generated_file = BufWriter::new(std::fs::File::create(
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("generated.rs"),
    )?);

    for testcase in test_driver_lib::collect_test_cases("cases")? {
        println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
        let mut module_name = testcase.identifier();
        if module_name.starts_with(|c: char| !c.is_ascii_alphabetic()) {
            module_name.insert(0, '_');
        }
        if let Some(style) = testcase.requested_style {
            module_name.push('_');
            module_name.push_str(style);
        }
        writeln!(generated_file, "#[path=\"{0}.rs\"] mod r#{0};", module_name)?;
        let source = std::fs::read_to_string(&testcase.absolute_path)?;
        let ignored = testcase.is_ignored("rust");

        let mut output = BufWriter::new(std::fs::File::create(
            Path::new(&std::env::var_os("OUT_DIR").unwrap()).join(format!("{}.rs", module_name)),
        )?);

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
#[test] {} fn t_{}() -> std::result::Result<(), std::boxed::Box<dyn std::error::Error>> {{
    use i_slint_backend_testing as slint_testing;
    slint_testing::init();
    {}
    Ok(())
}}",
                if ignored { "#[ignore]" } else { "" },
                i,
                x.source.replace('\n', "\n    ")
            )?;
        }
    }

    // By default resources are embedded. The WASM example builds provide test coverage for that. This switch
    // provides test coverage for the non-embedding case, compiling tests without embedding the images.
    println!("cargo:rustc-env=SLINT_EMBED_RESOURCES=false");

    //Make sure to use a consistent style
    println!("cargo:rustc-env=SLINT_STYLE=fluent");
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
    Ok(())
}

#[cfg(not(feature = "build-time"))]
fn generate_macro(
    source: &str,
    output: &mut dyn Write,
    testcase: test_driver_lib::TestCase,
) -> Result<bool, std::io::Error> {
    if source.contains("\\{") {
        // Unfortunately, \{ is not valid in a rust string so it cannot be used in a slint! macro
        output.write_all(b"#[test] #[ignore] fn ignored_because_string_template() {{}}")?;
        return Ok(false);
    }
    // to silence all the warnings in .slint files that would be turned into errors
    output.write_all(b"#![allow(deprecated)]")?;
    let include_paths = test_driver_lib::extract_include_paths(source);
    let library_paths = test_driver_lib::extract_library_paths(source);
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
    for (lib, path) in library_paths {
        let mut abs_path = testcase.absolute_path.clone();
        abs_path.pop();
        abs_path.push(path);

        output.write_all(b"#[library_path(")?;
        output.write_all(lib.as_bytes())?;
        output.write_all(b")=r#\"")?;
        output.write_all(abs_path.to_string_lossy().as_bytes())?;
        output.write_all(b"\"#]\n")?;

        println!("cargo:rerun-if-changed={}", abs_path.to_string_lossy());
    }

    if let Some(style) = testcase.requested_style {
        output.write_all(b"#[style=\"")?;
        output.write_all(style.as_bytes())?;
        output.write_all(b"\"#]\n")?;
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
    output: &mut impl Write,
    testcase: test_driver_lib::TestCase,
) -> Result<(), std::io::Error> {
    use i_slint_compiler::{diagnostics::BuildDiagnostics, *};

    let include_paths = test_driver_lib::extract_include_paths(source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    let library_paths = test_driver_lib::extract_library_paths(source)
        .map(|(k, v)| (k.to_string(), std::path::PathBuf::from(v)))
        .collect::<std::collections::HashMap<_, _>>();

    let mut diag = BuildDiagnostics::default();
    let syntax_node =
        parser::parse(source.to_owned(), Some(&testcase.absolute_path), None, &mut diag);
    let mut compiler_config = CompilerConfiguration::new(generator::OutputFormat::Rust);
    compiler_config.enable_component_containers = true;
    compiler_config.include_paths = include_paths;
    compiler_config.library_paths = library_paths;
    compiler_config.style = Some(testcase.requested_style.unwrap_or("fluent").to_string());
    let (root_component, diag, _) =
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
