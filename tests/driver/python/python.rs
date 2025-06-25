// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::{diagnostics::BuildDiagnostics, *};
use std::error::Error;
use std::io::Write;
use std::ops::Deref;

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;

    let include_paths = test_driver_lib::extract_include_paths(&source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    let library_paths = test_driver_lib::extract_library_paths(&source)
        .map(|(k, v)| (k.to_string(), std::path::PathBuf::from(v)))
        .collect::<std::collections::HashMap<_, _>>();

    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse(source.clone(), Some(&testcase.absolute_path), &mut diag);

    let mut compiler_config = CompilerConfiguration::new(generator::OutputFormat::Python);
    compiler_config.include_paths = include_paths;
    compiler_config.library_paths = library_paths;
    compiler_config.style = testcase.requested_style.map(str::to_string);
    compiler_config.debug_info = true;
    if source.contains("//bundle-translations") {
        compiler_config.translation_path_bundle =
            Some(testcase.absolute_path.parent().unwrap().to_path_buf());
        compiler_config.translation_domain =
            Some(testcase.absolute_path.file_stem().unwrap().to_str().unwrap().to_string());
    }
    let (root_component, diag, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));

    if diag.has_errors() {
        let vec = diag.to_string_vec();
        return Err(vec.join("\n").into());
    }

    // Check if there's any public API that we could generate an interface for
    let used_structs_or_enums = root_component.used_types.borrow().structs_and_enums.len();
    let globals = root_component
        .used_types
        .borrow()
        .globals
        .iter()
        .any(|glob| !glob.exported_global_names.borrow().is_empty());
    let components = root_component.exported_roots().count();

    let mut pyi_test_functions =
        test_driver_lib::extract_test_functions(&source).filter(|x| x.language_id == "pyi");

    let Some(expected_pyi) = pyi_test_functions.next().map(|f| {
        format!("# This file is auto-generated\n\nimport slint\nimport typing\n\n{}\n\n", f.source)
    }) else {
        return Ok(());
    };
    assert!(pyi_test_functions.next().is_none());

    let mut generated_python_interface: Vec<u8> = Vec::new();

    generator::generate(
        generator::OutputFormat::Python,
        &mut generated_python_interface,
        None,
        &root_component,
        &loader.compiler_config,
    )?;

    if diag.has_errors() {
        let vec = diag.to_string_vec();
        return Err(vec.join("\n").into());
    }

    let generated_python_interface = String::from_utf8(generated_python_interface).unwrap();

    assert_eq!(generated_python_interface, expected_pyi);

    Ok(())
}
