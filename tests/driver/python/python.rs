// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::{diagnostics::BuildDiagnostics, *};
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::sync::LazyLock;

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

    let mut generated_python_interface: Vec<u8> = Vec::new();
    let mut python_file = tempfile::Builder::new().suffix(".py").tempfile()?;

    generator::generate(
        generator::OutputFormat::Python,
        &mut generated_python_interface,
        Some(python_file.path()),
        &root_component,
        &loader.compiler_config,
    )?;

    assert!(!PYTHON_PATH.clone().as_os_str().is_empty());

    python_file
        .write(&generated_python_interface)
        .map_err(|err| format!("Error writing generated code: {err}"))?;
    python_file
        .as_file()
        .sync_all()
        .map_err(|err| format!("Error flushing generated code to disk: {err}"))?;
    let python_file = python_file.into_temp_path();

    let o = std::process::Command::new("uv")
        .arg("init")
        .arg("--script")
        .arg(&python_file)
        .arg("--python")
        .arg("3.12")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not launch uv init to add script tags: {err}"))
        .unwrap();
    check_output(o);

    let mut pyi_test_functions =
        test_driver_lib::extract_test_functions(&source).filter(|x| x.language_id == "pyi");

    if let Some(expected_pyi) = pyi_test_functions.next().map(|f| f.source.replace("\r\n", "\n")) {
        assert!(pyi_test_functions.next().is_none());

        let generated_python_interface = {
            let code = String::from_utf8(generated_python_interface).unwrap();
            let mut lines = code.trim_end().lines().collect::<Vec<_>>();

            let mut pop_front_if = |pattern| {
                if lines[0].starts_with(pattern) {
                    lines.remove(0);
                }
            };

            pop_front_if("# This file is auto-generated");
            pop_front_if("");
            pop_front_if("import slint");
            pop_front_if("import typing");
            pop_front_if("import enum");
            pop_front_if("import os");
            pop_front_if("");
            lines.pop(); // Remove call into slint package to load file
            lines.join("\n").trim_end().to_string()
        };

        assert_eq!(
            expected_pyi, generated_python_interface,
            "Generated API differed from expected.\nEXPECTED:\n{}\nACTUAL:\n{}\n",
            expected_pyi, generated_python_interface
        );

        if diag.has_errors() {
            let vec = diag.to_string_vec();
            return Err(vec.join("\n").into());
        }
    };

    let mut python_test_functions =
        test_driver_lib::extract_test_functions(&source).filter(|x| x.language_id == "python");

    if let Some(python_script) = python_test_functions.next().map(|f| f.source) {
        assert!(python_test_functions.next().is_none());

        // Append the python code to the bottom of the generated file and run it

        let mut f = std::fs::File::options().append(true).open(&python_file).unwrap();
        f.write(python_script.as_bytes()).unwrap();
    };

    let o = std::process::Command::new("uvx")
        .arg("--python")
        .arg("3.12")
        .arg("ty")
        .arg("check")
        .arg(&python_file)
        .env("PYTHONPATH", PYTHON_PATH.clone())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not launch uv ty check: {err}"))
        .unwrap();
    check_output(o);

    let o = std::process::Command::new("uv")
        .arg("run")
        .arg("--no-cache")
        .arg(&python_file)
        .env("PYTHONPATH", PYTHON_PATH.clone())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not launch uv run: {err}"))
        .unwrap();
    check_output(o);

    Ok(())
}

#[track_caller]
fn check_output(o: std::process::Output) {
    if !o.status.success() {
        eprintln!(
            "STDERR:\n{}\nSTDOUT:\n{}",
            String::from_utf8_lossy(&o.stderr),
            String::from_utf8_lossy(&o.stdout),
        );
        panic!("Process Failed {:?}", o.status);
    }
}

static PYTHON_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    let python_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../api/python/slint");

    // Sync env and build Slint
    check_output(
        std::process::Command::new("uv")
            .arg("sync")
            .current_dir(python_dir.clone())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|err| {
                format!("Could not launch uv init to set up environment for maturin: {err}")
            })
            .unwrap(),
    );

    python_dir
});
