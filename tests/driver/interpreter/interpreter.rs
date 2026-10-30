// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use itertools::Itertools;
use slint_interpreter::{DiagnosticLevel, Value, ValueType};
use std::{collections::HashMap, error::Error};

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    i_slint_backend_testing::init_no_event_loop();
    i_slint_backend_testing::configure_test_fonts();

    // Ensure we get consistent results on all platforms even for OS-dependent
    // behavior like dialog button order.
    i_slint_core::OPERATING_SYSTEM_OVERRIDE
        .set(Some(i_slint_core::items::OperatingSystemType::Windows));

    run_test(testcase, false)?;

    #[cfg(feature = "inject-debug-hooks")]
    run_test(testcase, true)?;

    #[cfg(feature = "llr-serialization")]
    run_test_serialized_llr(testcase)?;

    Ok(())
}

/// A compiler configured for `testcase`, whose contents are `source`.
fn new_compiler(
    testcase: &test_driver_lib::TestCase,
    source: &str,
) -> Result<slint_interpreter::Compiler, Box<dyn Error>> {
    let include_paths = test_driver_lib::extract_include_paths(source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    let library_paths = test_driver_lib::extract_library_paths(source)
        .map(|(k, v)| (k.to_string(), std::path::PathBuf::from(v)))
        .collect::<HashMap<_, _>>();
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_include_paths(include_paths);
    compiler.set_library_paths(library_paths);
    compiler.set_style(testcase.requested_style.unwrap_or("fluent").into());
    let compiler_config = compiler.compiler_configuration(i_slint_core::InternalToken);
    compiler_config.edition = Some(test_driver_lib::edition_or_current(source).parse()?);
    Ok(compiler)
}

fn run_test(
    testcase: &test_driver_lib::TestCase,
    inject_debug_hooks: bool,
) -> Result<(), Box<dyn Error>> {
    if inject_debug_hooks && testcase.is_ignored("interpreter-debug-hooks") {
        eprintln!("Ignoring test {} for the debug hooks", testcase.relative_path.display());
        return Ok(());
    }

    let mode = if inject_debug_hooks { " (with debug hooks)" } else { "" };

    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let mut compiler = new_compiler(testcase, &source)?;

    if inject_debug_hooks {
        compiler.compiler_configuration(i_slint_core::InternalToken).debug_hooks =
            Some(std::hash::RandomState::new());
    }

    let result =
        spin_on::spin_on(compiler.build_from_source(source, testcase.absolute_path.clone()));

    if result.has_errors() {
        let diagnostics = result.diagnostics().collect::<Vec<_>>();
        slint_interpreter::print_diagnostics(&diagnostics);

        match std::env::var("SLINT_INTERPRETER_ERROR_WHITELIST") {
            Ok(wl) if !wl.is_empty() => {
                let errors = diagnostics
                    .iter()
                    .filter(|d| d.level() == DiagnosticLevel::Error)
                    .collect::<Vec<_>>();
                if !errors.is_empty()
                    && errors.iter().all(|d| wl.split(';').any(|w| d.message().contains(w)))
                {
                    eprintln!(
                        "{}: Ignoring Error because of the error whitelist!",
                        testcase.relative_path.display()
                    );
                    return Ok(());
                }
            }
            _ => {}
        }

        return Err(format!(
            "compilation failed{mode}:\n{}",
            diagnostics.iter().map(|d| d.to_string()).join("\n")
        )
        .into());
    }

    process_result(testcase, &result, mode)
}

/// Checks the `test` property of every component of `result`.
fn process_result(
    testcase: &test_driver_lib::TestCase,
    result: &slint_interpreter::CompilationResult,
    mode: &str,
) -> Result<(), Box<dyn Error>> {
    for component_name in result.component_names() {
        let component = result.component(component_name).unwrap();
        let instance = component.create().unwrap();

        if let Some((_, ty)) = component.properties().find(|x| x.0 == "test") {
            if ty == ValueType::Bool {
                let result = instance.get_property("test")?;
                if result != Value::Bool(true) {
                    eprintln!(
                        "FAIL: {}{mode}: test returned {:?}",
                        testcase.relative_path.display(),
                        result
                    );
                    eprintln!("Property list:");
                    for (p, _) in component.properties() {
                        eprintln!(" {}: {:?}", p, instance.get_property(&p));
                    }
                    panic!("Test Failed{mode}: {result:?}");
                }
            }
        };
    }

    Ok(())
}

/// Runs `testcase` on a unit that went through serialization.
#[cfg(feature = "llr-serialization")]
fn run_test_serialized_llr(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    use i_slint_compiler::llr::serialize;
    use i_slint_core::InternalToken;
    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let mut compiler = new_compiler(testcase, &source)?;
    let mut config = compiler.compiler_configuration(InternalToken).clone();
    // The compiler keeps these as overrides, outside of its configuration
    config.include_paths = compiler.include_paths().clone();
    config.library_paths = compiler.library_paths().clone();
    config.style = compiler.style().cloned();
    let path = i_slint_compiler::source_path::SourcePath::File(testcase.absolute_path.clone());
    let (diagnostics, compiled) = spin_on::spin_on(serialize::compile(&path, source, config));
    let mode = " (with a serialized LLR)";
    let (unit, resources) = compiled.ok_or_else(|| {
        format!("compilation failed{mode}:\n{}", diagnostics.to_string_vec().join("\n"))
    })?;
    let unit = serialize::from_bytes(&serialize::to_bytes(&unit)?)?;
    let resources = resources
        .iter()
        .map(|resource| {
            let resource =
                serialize::resource_from_bytes(&serialize::resource_to_bytes(resource)?)?;
            Ok(slint_interpreter::LoadedResource::new(resource, InternalToken))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let result =
        slint_interpreter::CompilationResult::from_compilation_unit(unit, resources, InternalToken);
    process_result(testcase, &result, mode)
}
