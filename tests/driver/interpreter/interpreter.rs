// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use itertools::Itertools;
use slint_interpreter::{DiagnosticLevel, Value, ValueType};
use std::{collections::HashMap, error::Error};

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    i_slint_backend_testing::init_no_event_loop();

    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let include_paths = test_driver_lib::extract_include_paths(&source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    let library_paths = test_driver_lib::extract_library_paths(&source)
        .map(|(k, v)| (k.to_string(), std::path::PathBuf::from(v)))
        .collect::<HashMap<_, _>>();
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_include_paths(include_paths);
    compiler.set_library_paths(library_paths);
    compiler.set_style(testcase.requested_style.unwrap_or("fluent").into());

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

        return Err(diagnostics.iter().map(|d| d.to_string()).join("\n").into());
    }

    for component_name in result.component_names() {
        let component = result.component(component_name).unwrap();
        let instance = component.create().unwrap();

        if let Some((_, ty)) = component.properties().find(|x| x.0 == "test") {
            if ty == ValueType::Bool {
                let result = instance.get_property("test")?;
                if result != Value::Bool(true) {
                    eprintln!(
                        "FAIL: {}: test returned {:?}",
                        testcase.relative_path.display(),
                        result
                    );
                    eprintln!("Property list:");
                    for (p, _) in component.properties() {
                        eprintln!(" {}: {:?}", p, instance.get_property(&p));
                    }
                    panic!("Test Failed: {result:?}");
                }
            }
        };
    }

    Ok(())
}
