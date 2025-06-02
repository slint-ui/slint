// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This test is trying to compile all the *.slint files in the sub directories and check that compilation
//! errors are properly reported
//!
//! The .slint files can have comments like this:
//! ```ignore
//!  hi ho
//!  // ^error{expected_message}
//! ```
//!
//! Meaning that there must an error with that error message in the position on the line above at the column pointed by the caret.
//! If there are two carets: ` ^^error{expected_message}`  then it means two line above, and so on with more carets.
//! `^warning{expected_message}` is also supported.
//!
//! The newlines are replaced by `↵` in the error message. Also the manifest dir (CARGO_MANIFEST_DIR) is replaced by `📂`.
//!
//! When the env variable `SLINT_SYNTAX_TEST_UPDATE` is set to `1`, the source code will be modified to add the comments

use i_slint_compiler::diagnostics::{BuildDiagnostics, Diagnostic, DiagnosticLevel};
use i_slint_compiler::ComponentSelection;
use std::path::{Path, PathBuf};

#[test]
fn syntax_tests() -> std::io::Result<()> {
    use rayon::prelude::*;

    let update = std::env::args().any(|arg| arg == "--update")
        || std::env::var("SLINT_SYNTAX_TEST_UPDATE").is_ok_and(|v| v == "1");

    if let Some(specific_test) = std::env::args()
        .skip(1)
        .skip_while(|arg| arg.starts_with("--") || arg == "syntax_tests")
        .next()
    {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push(specific_test);
        assert!(process_file(&path, update)?);
        return Ok(());
    }

    let mut test_entries = Vec::new();
    for entry in std::fs::read_dir(format!("{}/tests/syntax", env!("CARGO_MANIFEST_DIR")))? {
        let entry = entry?;
        if entry.file_type().is_ok_and(|f| f.is_dir()) {
            let path = entry.path();
            for test_entry in path.read_dir()? {
                let test_entry = test_entry?;
                let path = test_entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "60" || ext == "slint" {
                        test_entries.push(path);
                    }
                }
            }
        }
    }

    let success = test_entries
        .par_iter()
        .try_fold(
            || true,
            |mut success, path| {
                success &= process_file(path, update)?;
                Ok::<bool, std::io::Error>(success)
            },
        )
        .try_reduce(|| true, |success, result| Ok(success & result))?;

    assert!(success);

    Ok(())
}

fn process_file(path: &std::path::Path, update: bool) -> std::io::Result<bool> {
    let source = std::fs::read_to_string(path)?;
    if path.to_str().unwrap_or("").contains("bom-") && !source.starts_with("\u{FEFF}") {
        // make sure that the file still contains BOM and it wasn't remove by some tools
        return Err(std::io::Error::other(format!(
            "{path:?} does not contains BOM while it should"
        )));
    }
    std::panic::catch_unwind(|| process_file_source(path, source, false, update)).unwrap_or_else(
        |err| {
            println!("Panic while processing {}: {:?}", path.display(), err);
            Ok(false)
        },
    )
}

fn process_diagnostics(
    compile_diagnostics: &BuildDiagnostics,
    path: &Path,
    source: &str,
    _silent: bool,
    update: bool,
) -> std::io::Result<bool> {
    let mut success = true;

    let path = canonical(path);

    let mut diags = compile_diagnostics
        .iter()
        .filter(|d| {
            canonical(
                d.source_file()
                    .unwrap_or_else(|| panic!("{path:?}: Error without a source file {d:?}",)),
            ) == path
        })
        .collect::<Vec<_>>();

    let lines = source
        .bytes()
        .enumerate()
        .filter_map(|(i, c)| if c == b'\n' { Some(i) } else { None })
        .collect::<Vec<usize>>();

    let diag_copy = diags.clone();
    let mut captures = Vec::new();

    // Find expected errors in the file. The first caret (^) points to the expected column. The number of
    // carets refers to the number of lines to go back. This is useful when one line of code produces multiple
    // errors or warnings.
    let re = regex::Regex::new(r"\n *//[^\n\^]*(\^+)(error|warning)\{([^\n]*)\}").unwrap();
    for m in re.captures_iter(source) {
        let line_begin_offset = m.get(0).unwrap().start();
        let column = m.get(1).unwrap().start() - line_begin_offset;
        let lines_to_source = m.get(1).unwrap().as_str().len();
        let warning_or_error = m.get(2).unwrap().as_str();
        let expected_message =
            m.get(3).unwrap().as_str().replace('↵', "\n").replace('📂', env!("CARGO_MANIFEST_DIR"));
        if update {
            captures.push(m.get(0).unwrap().range());
        }

        let mut line_counter = 0;
        let mut line_offset = source[..line_begin_offset].rfind('\n').unwrap_or(0);
        let offset = loop {
            line_counter += 1;
            if line_counter >= lines_to_source {
                break line_offset + column;
            }
            if let Some(o) = source[..line_offset].rfind('\n') {
                line_offset = o;
            } else {
                break 1;
            };
        };

        let expected_diag_level = match warning_or_error {
            "warning" => DiagnosticLevel::Warning,
            "error" => DiagnosticLevel::Error,
            _ => panic!("Unsupported diagnostic level {warning_or_error}"),
        };

        fn compare_message(message: &str, expected_message: &str) -> bool {
            if message == expected_message {
                return true;
            }
            // The error message might contain path that might have other character, so replace them on windows
            #[cfg(target_os = "windows")]
            if message.replace('\\', "/") == expected_message.replace('\\', "/") {
                return true;
            }
            false
        }

        match diags.iter().position(|e| {
            let (l, c) = e.line_column();
            let o = lines.get(l.wrapping_sub(2)).unwrap_or(&0) + c;
            o == offset
                && compare_message(e.message(), &expected_message)
                && e.level() == expected_diag_level
        }) {
            Some(idx) => {
                diags.remove(idx);
            }
            None => {
                success = false;
                println!("{path:?}: {warning_or_error} not found at offset {offset}: {expected_message:?}");
            }
        }
    }

    if !diags.is_empty() {
        println!("{path:?}: Unexpected errors/warnings: {diags:#?}");

        #[cfg(feature = "display-diagnostics")]
        if !_silent {
            let mut to_report = BuildDiagnostics::default();
            for d in diags {
                to_report.push_compiler_error(d.clone());
            }
            to_report.print();
        }

        success = false;
    }

    if !success && update {
        let mut source = source.to_string();
        self::update(diag_copy, &mut source, lines, &captures);
        std::fs::write(path, source).unwrap();
    }

    Ok(success)
}

/// Rewrite the source to remove the old comments and add accurate error comments
fn update(
    mut diags: Vec<&Diagnostic>,
    source: &mut String,
    mut lines: Vec<usize>,
    to_remove: &[std::ops::Range<usize>],
) {
    for to_remove in to_remove.iter().rev() {
        source.drain(to_remove.clone());
        for l in &mut lines {
            if *l > to_remove.start {
                *l -= to_remove.end - to_remove.start;
            }
        }
    }

    diags.sort_by_key(|d| {
        let (l, c) = d.line_column();
        (usize::MAX - l, c)
    });

    let mut last_line = 0;
    let mut last_line_adjust = 0;

    for d in diags {
        let (l, c) = d.line_column();
        if c < 3 {
            panic!("Error message cannot be on the column < 3: {d:?}")
        }

        if last_line == l {
            last_line_adjust += 1;
        } else {
            last_line = l;
            last_line_adjust = 0;
        }

        let byte_offset = lines[l - 1] + 1;

        let to_insert = format!(
            "//{indent}^{adjust}{error_or_warning}{{{message}}}\n",
            indent = " ".repeat(c - 3),
            adjust = "^".repeat(last_line_adjust),
            error_or_warning =
                if d.level() == DiagnosticLevel::Error { "error" } else { "warning" },
            message = d.message().replace('\n', "↵").replace(env!("CARGO_MANIFEST_DIR"), "📂")
        );
        if byte_offset > source.len() {
            source.push('\n');
        }
        source.insert_str(byte_offset, &to_insert);
        lines[l - 1] += to_insert.len();
    }
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}

fn process_file_source(
    path: &std::path::Path,
    source: String,
    silent: bool,
    update: bool,
) -> std::io::Result<bool> {
    let mut parse_diagnostics = BuildDiagnostics::default();
    let syntax_node =
        i_slint_compiler::parser::parse(source.clone(), Some(path), &mut parse_diagnostics);

    let has_parse_error = parse_diagnostics.has_errors();
    let mut compiler_config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    compiler_config.embed_resources = i_slint_compiler::EmbedResourcesKind::OnlyBuiltinResources;
    compiler_config.enable_experimental = true;
    compiler_config.style = Some("fluent".into());
    compiler_config.components_to_generate =
        if source.contains("config:generate_all_exported_windows") {
            ComponentSelection::ExportedWindows
        } else {
            // Otherwise we'd have lots of warnings about not inheriting Window
            ComponentSelection::LastExported
        };
    let compile_diagnostics = if !parse_diagnostics.has_errors() {
        let (_, build_diags, _) = spin_on::spin_on(i_slint_compiler::compile_syntax_node(
            syntax_node.clone(),
            parse_diagnostics,
            compiler_config.clone(),
        ));
        build_diags
    } else {
        parse_diagnostics
    };

    let mut success = true;
    success &= process_diagnostics(&compile_diagnostics, path, &source, silent, update)?;

    for p in &compile_diagnostics.all_loaded_files {
        let source = if p.is_absolute() {
            std::fs::read_to_string(p)?
        } else {
            // probably std-widgets.slint
            String::new()
        };
        success &= process_diagnostics(&compile_diagnostics, p, &source, silent, update)?;
    }

    if has_parse_error {
        // Still try to compile to make sure it doesn't panic
        spin_on::spin_on(i_slint_compiler::compile_syntax_node(
            syntax_node,
            compile_diagnostics,
            compiler_config,
        ));
    }

    Ok(success)
}

#[test]
/// Test that this actually fail when it should
fn self_test() -> std::io::Result<()> {
    let fake_path = std::path::Path::new("fake.slint");
    let process = |str: &str| process_file_source(fake_path, str.into(), true, false);

    // this should succeed
    assert!(process(
        r#"
export component Foo inherits Window { width: 10px; }
    "#
    )?);

    // unless we expected an error
    assert!(!process(
        r#"
export component Foo inherits Window { width: 10px; }
//            ^error{i want an error}
    "#
    )?);

    // An error should fail
    assert!(!process(
        r#"
export component Foo inherits Window foo { width: 10px; }
    "#
    )?);

    // An error with the proper comment should pass
    assert!(process(
        r#"
export component Foo inherits Window foo { width: 10px; }
//                                   ^error{Syntax error: expected '{'}
    "#
    )?);

    // But not if it is at the wrong position
    assert!(!process(
        r#"
export component Foo inherits Window foo { width: 10px; }
//                                    ^error{Syntax error: expected '{'}
    "#
    )?);

    // or the wrong line
    assert!(!process(
        r#"
export component Foo inherits Window foo { width: 10px; }

//                                   ^error{Syntax error: expected '{'}
    "#
    )?);

    // or the wrong message
    assert!(!process(
        r#"
export component Foo inherits Window foo { width: 10px; }
//                                   ^error{foo_bar}
    "#
    )?);

    // or the wrong line because two carets
    assert!(!process(
        r#"

export component Foo inherits Window foo { width: 10px; }
//                                   ^^error{Syntax error: expected '{'}
    "#
    )?);

    // Even on windows, it should work
    assert!(process(
        "\r\nexport component Foo inherits Window foo { width: 10px; }\r\n//                                   ^error{Syntax error: expected '{'}\r\n"
    )?);

    Ok(())
}
