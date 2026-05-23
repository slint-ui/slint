// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::{diagnostics::BuildDiagnostics, *};
use std::error::Error;
use std::path::Path;

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

    let mut compiler_config = CompilerConfiguration::new(generator::OutputFormat::TypeScript);
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

    let mut generated_ts: Vec<u8> = Vec::new();

    generator::generate(
        generator::OutputFormat::TypeScript,
        &mut generated_ts,
        None,
        &root_component,
        &loader.compiler_config,
    )?;

    let mut ts_test_functions =
        test_driver_lib::extract_test_functions(&source).filter(|x| x.language_id == "d.ts");

    if let Some(expected_ts) = ts_test_functions.next().map(|f| f.source.replace("\r\n", "\n")) {
        assert!(ts_test_functions.next().is_none());

        let generated_ts_interface = {
            let code = String::from_utf8(generated_ts).unwrap();
            let mut lines = code.trim_end().lines().collect::<Vec<_>>();

            let mut pop_front_if = |pattern: &str| {
                if !lines.is_empty() && lines[0].starts_with(pattern) {
                    lines.remove(0);
                }
            };

            pop_front_if("// This file is auto-generated");
            pop_front_if("");
            pop_front_if("import ");
            pop_front_if("");
            lines.join("\n").trim_end().to_string()
        };

        assert_eq!(expected_ts, generated_ts_interface);
    };

    Ok(())
}

/// Compile a .slint file and generate .d.ts output.
/// Returns `None` if the file has compilation errors (expected for some test cases).
fn generate_dts(slint_path: &Path, dest_path: &Path) -> Option<Vec<u8>> {
    let source = std::fs::read_to_string(slint_path).ok()?;

    let include_paths = test_driver_lib::extract_include_paths(&source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    let library_paths = test_driver_lib::extract_library_paths(&source)
        .map(|(k, v)| (k.to_string(), std::path::PathBuf::from(v)))
        .collect::<std::collections::HashMap<_, _>>();

    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse(source.clone(), Some(slint_path), &mut diag);

    let mut compiler_config = CompilerConfiguration::new(generator::OutputFormat::TypeScript);
    compiler_config.include_paths = include_paths;
    compiler_config.library_paths = library_paths;
    if source.contains("//bundle-translations") {
        compiler_config.translation_path_bundle = Some(slint_path.parent().unwrap().to_path_buf());
        compiler_config.translation_domain =
            Some(slint_path.file_stem().unwrap().to_str().unwrap().to_string());
    }

    let (root_component, diag, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));

    if diag.has_errors() {
        return None;
    }

    let mut output = Vec::new();
    generator::generate(
        generator::OutputFormat::TypeScript,
        &mut output,
        Some(dest_path),
        &root_component,
        &loader.compiler_config,
    )
    .ok()?;

    Some(output)
}

/// Generate .d.ts files for all test cases and run `tsc --noEmit` to validate
/// that the generated declarations are consistent with the real slint-ui types.
pub fn typecheck_all(paths: &[&str]) {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("can't find workspace root");

    let tsc = workspace_root.join("node_modules/.bin/tsc");
    if !tsc.exists() {
        eprintln!("Skipping typecheck: tsc not found at {}", tsc.display());
        return;
    }

    let slint_ui_types = workspace_root.join("api/node/dist/index.d.ts");
    if !slint_ui_types.exists() {
        eprintln!("Skipping typecheck: slint-ui types not found at {}", slint_ui_types.display());
        return;
    }

    let tmp_dir = std::env::temp_dir().join(format!("slint-ts-typecheck-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let mut file_count = 0u32;

    for path_str in paths {
        let slint_path = Path::new(path_str);
        // Pass a .d.ts path to the generator so it emits `declare` syntax,
        // but save with a .ts extension so tsc checks it (skipLibCheck skips .d.ts)
        let dts_path = tmp_dir.join(format!("test_{file_count}.slint.d.ts"));
        let ts_path = tmp_dir.join(format!("test_{file_count}.slint.ts"));

        if let Some(output) = generate_dts(slint_path, &dts_path) {
            std::fs::write(&ts_path, output).unwrap();
            file_count += 1;
        }
    }

    if file_count == 0 {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        panic!("No .d.ts files were generated — something is wrong with the test setup");
    }

    // Write tsconfig.json pointing to the real slint-ui type definitions
    let tsconfig = format!(
        r#"{{
    "compilerOptions": {{
        "module": "esnext",
        "moduleResolution": "bundler",
        "strict": true,
        "noEmit": true,
        "skipLibCheck": true,
        "paths": {{ "slint-ui": ["{}"] }}
    }},
    "include": ["*.ts"]
}}"#,
        slint_ui_types.to_string_lossy().replace('\\', "/")
    );
    std::fs::write(tmp_dir.join("tsconfig.json"), tsconfig).unwrap();

    let output = std::process::Command::new(&tsc)
        .arg("--noEmit")
        .current_dir(&tmp_dir)
        .output()
        .expect("failed to run tsc");

    let _ = std::fs::remove_dir_all(&tmp_dir);

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("tsc type checking failed ({file_count} files generated):\n{stdout}{stderr}");
    }

    eprintln!("typecheck passed for {file_count} generated .d.ts files");
}
