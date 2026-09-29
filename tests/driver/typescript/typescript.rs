// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::{diagnostics::BuildDiagnostics, *};
use std::error::Error;
use std::path::Path;

/// Compile a `.slint` file and return the declarations generated for it.
fn generate_dts(path: &Path) -> Result<Vec<u8>, String> {
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;

    let mut compiler_config = CompilerConfiguration::new(generator::OutputFormat::TypeScript);
    compiler_config.include_paths =
        test_driver_lib::extract_include_paths(&source).map(std::path::PathBuf::from).collect();
    compiler_config.library_paths = test_driver_lib::extract_library_paths(&source)
        .map(|(k, v)| (k.to_string(), std::path::PathBuf::from(v)))
        .collect();
    compiler_config.debug_info = true;

    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse(source, Some(path), &mut diag);
    let (root_component, diag, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));

    if diag.has_errors() {
        return Err(diag.to_string_vec().join("\n"));
    }

    let mut generated = Vec::new();
    generator::generate(
        generator::OutputFormat::TypeScript,
        &mut generated,
        None,
        &root_component,
        &loader.compiler_config,
    )
    .map_err(|e| e.to_string())?;
    Ok(generated)
}

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let generated = generate_dts(&testcase.absolute_path)?;

    let mut expected =
        test_driver_lib::extract_test_functions(&source).filter(|x| x.language_id == "d.ts");

    if let Some(expected_ts) = expected.next().map(|f| f.source.replace("\r\n", "\n")) {
        assert!(expected.next().is_none());
        assert_eq!(expected_ts, strip_preamble(&generated));
    };

    Ok(())
}

/// Everything after the header and the `import`, which the expected blocks leave out.
fn strip_preamble(generated: &[u8]) -> String {
    let code = String::from_utf8(generated.to_vec()).unwrap();
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
}

/// Generate the declarations for every test case and run `tsc --noEmit` over them, to check
/// they are consistent with the real slint-ui types.
pub fn typecheck_all(paths: &[&str]) {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("can't find workspace root");

    // Run tsc through node rather than the node_modules/.bin wrapper:
    // the wrapper is a shell script that Command::new can't spawn on Windows.
    let tsc = workspace_root.join("node_modules/typescript/lib/tsc.js");
    assert!(tsc.exists(), "{} is missing, run `pnpm install`", tsc.display());
    let slint_ui_types = workspace_root.join("api/node/dist/index.d.ts");
    assert!(
        slint_ui_types.exists(),
        "{} is missing, run `pnpm -C api/node build`",
        slint_ui_types.display()
    );

    let tmp_dir = tempfile::tempdir().unwrap();

    // Compiling every case takes a while, so spread it over the cores. Saved with a `.ts`
    // extension so that tsc checks it: `skipLibCheck` would skip a `.d.ts`.
    let chunks = std::thread::available_parallelism().map_or(1, |n| n.get());
    let generated: Vec<Vec<u8>> = std::thread::scope(|scope| {
        paths
            .chunks(paths.len().div_ceil(chunks))
            .map(|chunk| {
                scope.spawn(|| {
                    chunk.iter().filter_map(|p| generate_dts(Path::new(p)).ok()).collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect()
    });
    for (i, declarations) in generated.iter().enumerate() {
        std::fs::write(tmp_dir.path().join(format!("test_{i}.slint.ts")), declarations).unwrap();
    }
    let file_count = generated.len();
    assert!(file_count > 0, "No declarations were generated — something is wrong with the setup");

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
    std::fs::write(tmp_dir.path().join("tsconfig.json"), tsconfig).unwrap();

    let output = std::process::Command::new("node")
        .arg(&tsc)
        .arg("--noEmit")
        .current_dir(tmp_dir.path())
        .output()
        .expect("failed to run tsc");

    assert!(
        output.status.success(),
        "tsc type checking failed ({file_count} files generated):\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    eprintln!("typecheck passed for {file_count} generated declaration files");
}
