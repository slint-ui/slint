// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::io::Write;
use std::path::Path;

/// Returns a list of all the `.slint` files in the `tests/cases` subfolders.
pub fn collect_test_cases() -> std::io::Result<Vec<test_driver_lib::TestCase>> {
    let mut results = vec![];

    let case_root_dir: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "cases"].iter().collect();

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
    let default_font_path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "..", "..", "examples", "printerdemo", "ui", "fonts"]
            .iter()
            .collect();

    std::env::set_var("SLINT_DEFAULT_FONT", default_font_path.clone());
    println!("cargo:rustc-env=SLINT_DEFAULT_FONT={}", default_font_path.display());

    let mut generated_file = std::fs::File::create(
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("generated.rs"),
    )?;

    let references_root_dir: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "references"].iter().collect();

    for (i, testcase) in
        test_driver_lib::collect_test_cases("screenshots/cases")?.into_iter().enumerate()
    {
        let mut reference_path = references_root_dir
            .join(testcase.relative_path.clone())
            .with_extension("png")
            .to_str()
            .unwrap()
            .escape_default()
            .to_string();

        reference_path = format!("\"{}\"", reference_path);

        println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
        let mut module_name = testcase.identifier();
        if module_name.starts_with(|c: char| !c.is_ascii_alphabetic()) {
            module_name.insert(0, '_');
        }
        writeln!(generated_file, "#[path=\"{0}.rs\"] mod r#{0};", module_name)?;
        let source = std::fs::read_to_string(&testcase.absolute_path)?;

        let needle = "SLINT_SCALE_FACTOR=";
        let scale_factor = if let Some(p) = source.find(needle) {
            let source = &source[p + needle.len()..];
            let scale_factor: f32 = source
                .find(char::is_whitespace)
                .and_then(|end| source[..end].parse().ok())
                .unwrap_or_else(|| {
                    panic!("Cannot parse {needle} for {}", testcase.relative_path.display())
                });
            format!("slint::platform::WindowAdapter::window(&*window).dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {{ scale_factor: {scale_factor}f32 }});")
        } else {
            String::new()
        };

        let mut output = std::fs::File::create(
            Path::new(&std::env::var_os("OUT_DIR").unwrap()).join(format!("{}.rs", module_name)),
        )?;

        generate_source(source.as_str(), &mut output, testcase).unwrap();

        write!(
            output,
            r"
    #[test] fn t_{}() -> Result<(), Box<dyn std::error::Error>> {{
    use crate::testing;

    let window = testing::init_swr();
    {scale_factor}
    window.set_size(slint::PhysicalSize::new(64, 64));
    let screenshot = {reference_path};

    let instance = TestCase::new().unwrap();
    instance.show().unwrap();

    testing::assert_with_render(screenshot, window.clone());

    testing::assert_with_render_by_line(screenshot, window.clone());

    Ok(())
    }}",
            i,
        )?;
    }

    //Make sure to use a consistent style
    println!("cargo:rustc-env=SLINT_STYLE=fluent");
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");

    Ok(())
}

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
    let syntax_node = parser::parse(source.to_owned(), Some(&testcase.absolute_path), &mut diag);
    let mut compiler_config = CompilerConfiguration::new(generator::OutputFormat::Rust);
    compiler_config.include_paths = include_paths;
    compiler_config.embed_resources = EmbedResourcesKind::EmbedTextures;
    compiler_config.enable_component_containers = true;
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
