// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This test is trying to compile all the *.slint files in the sub directories and check that compilation
//! errors are properly reported.
//!
//! The .slint files can have comments like this:
//! ```ignore
//!  hi foo
//!  // > <error{expected_message}
//! ```
//!
//! Meaning that there must an error with that error message spanning the characters on the line above between the `>` and `<` characters.
//! The `>` and `<` indicators may also appear individually, if the diagnostic spans multiple lines.
//!
//! A `^` character means that the diagnostic must only span that single character.
//! A `|` character means that the diagnostic must return a length of 0 and not span any
//! characters (although most LSP clients will render it as spanning at least 1 character).
//!
//! If there are additional `^` characters: `> <^error{expected_message}`  then it means the comment refers to a diagnostic two lines above, instead of one, and so on with more carets.
//!
//! If there are additional `<` characters: `> <<error{expected_message}` then it means the range
//! should be an additional character to the left, which is useful if the diagnostic starts or ends
//! in the first or second column, where otherwise the `//` is located.
//!
//! Warnings with `> <warning{expected_message}` are also supported.
//!
//! The newlines are replaced by `↵` in the error message. Also the manifest dir (CARGO_MANIFEST_DIR) is replaced by `📂`.
//!
//! When the env variable `SLINT_SYNTAX_TEST_UPDATE` is set to `1`, the source code will be modified to add the comments
//! The env variable `SLINT_TEST_FILTER` accepts a regexp and will filter out tests not maching that pattern

use i_slint_compiler::ComponentSelection;
use i_slint_compiler::diagnostics::{
    self, BuildDiagnostics, ByteFormat, Diagnostic, DiagnosticLevel,
};
use std::ops::Range;
use std::path::{Path, PathBuf};

#[test]
fn syntax_tests() -> std::io::Result<()> {
    use rayon::prelude::*;

    let update = std::env::args().any(|arg| arg == "--update")
        || std::env::var("SLINT_SYNTAX_TEST_UPDATE").is_ok_and(|v| v == "1");

    if let Some(specific_test) =
        std::env::args().skip(1).find(|arg| !(arg.starts_with("--") || arg == "syntax_tests"))
    {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push(specific_test);
        assert!(process_file(&path, update)?);
        return Ok(());
    }

    let pattern = std::env::var("SLINT_TEST_FILTER").ok().map(|p| regex::Regex::new(&p).unwrap());

    let mut test_entries = Vec::new();
    for entry in std::fs::read_dir(format!("{}/tests/syntax", env!("CARGO_MANIFEST_DIR")))? {
        let entry = entry?;
        if entry.file_type().is_ok_and(|f| f.is_dir()) {
            let path = entry.path();
            for test_entry in path.read_dir()? {
                let test_entry = test_entry?;
                let path = test_entry.path();
                if let Some(ext) = path.extension() {
                    if (ext == "60" || ext == "slint")
                        && pattern
                            .as_ref()
                            .map(|p| p.is_match(&path.to_string_lossy()))
                            .unwrap_or(true)
                    {
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

struct ExpectedDiagnostic {
    start: Option<usize>,
    end: Option<usize>,
    level: DiagnosticLevel,
    message: String,
    comment_range: Range<usize>,
}

fn extract_expected_diags(source: &str) -> Vec<ExpectedDiagnostic> {
    let mut expected = Vec::new();
    // Find expected errors in the file. The first caret (^) points to the expected column. The number of
    // carets refers to the number of lines to go back. This is useful when one line of code produces multiple
    // errors or warnings.
    let re = regex::Regex::new(
        r"\n *//[^\n\^\|<>]*((\^)|(\|)|((>)?( *<)?))(\^*)(<*)(error|warning|note)\{([^\n]*)\}",
    )
    .unwrap();

    for m in re.captures_iter(source) {
        let line_begin_offset = m.get(0).unwrap().start();
        let start_column = m.get(1).unwrap().start()
            - line_begin_offset
            // Allow shifting columns with <
            - m.get(8).map(|group| group.as_str().len()).unwrap_or_default();

        let lines_to_source = m.get(7).map(|group| group.as_str().len()).unwrap_or_default() + 1;
        let warning_or_error = m.get(9).unwrap().as_str();
        let expected_message = m
            .get(10)
            .unwrap()
            .as_str()
            .replace('↵', "\n")
            .replace('📂', env!("CARGO_MANIFEST_DIR"));
        let comment_range = m.get(0).unwrap().range();

        let mut line_counter = 0;
        let mut line_offset = source[..line_begin_offset].rfind('\n').unwrap_or(0);
        let mut offset = loop {
            line_counter += 1;
            if line_counter >= lines_to_source {
                break line_offset + start_column;
            }
            if let Some(o) = source[..line_offset].rfind('\n') {
                line_offset = o;
            } else {
                break 1;
            };
        };

        let mut start = None;
        let mut end = None;
        if m.get(2).is_some() {
            // ^warning{...}
            start = Some(offset);
            end = Some(offset);
        } else if m.get(3).is_some() {
            // |warning{...}
            start = Some(offset);
            end = Some(offset - 1);
        } else {
            // >
            if m.get(5).is_some() {
                start = Some(offset);
                offset += 1;
            }
            // < (including spaces before)
            if let Some(range_length) = m.get(6).map(|group| group.as_str().len()) {
                end = Some(offset + range_length - 1);
            }
        }

        // Windows edge-case, if the end falls on a newline, it should span the entire
        // newline character, which is two characters, not one.
        if let Some(end_offset) = end {
            if source.get(end_offset..=(end_offset + 1)) == Some("\r\n") {
                end = Some(end_offset + 1)
            };
        }

        let expected_diag_level = match warning_or_error {
            "warning" => DiagnosticLevel::Warning,
            "error" => DiagnosticLevel::Error,
            "note" => DiagnosticLevel::Note,
            _ => panic!("Unsupported diagnostic level {warning_or_error}"),
        };

        expected.push(ExpectedDiagnostic {
            start,
            end,
            level: expected_diag_level,
            message: expected_message,
            comment_range,
        });
    }
    expected
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

    let diags = compile_diagnostics
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

    let mut expected = extract_expected_diags(source);
    let captures: Vec<_> =
        expected.iter().map(|expected| &expected.comment_range).cloned().collect();

    // Find expected errors in the file. The first caret (^) points to the expected column. The number of
    // carets refers to the number of lines to go back. This is useful when one line of code produces multiple
    // errors or warnings.
    for diag in &diags {
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

        let (l, c) = diag.line_column();
        let diag_start = lines.get(l.wrapping_sub(2)).unwrap_or(&0) + c;
        // end_line_column is not (yet) available via the public API, so use the private API
        // instead.
        let (l, c) = diagnostics::diagnostic_end_line_column_with_format(diag, ByteFormat::Utf8);
        let diag_end = lines.get(l.wrapping_sub(2)).unwrap_or(&0) + c - 1;

        let expected_start = expected.iter().position(|expected| {
            Some(diag_start) == expected.start
                && compare_message(diag.message(), &expected.message)
                && diag.level() == expected.level
                && (expected.end.is_none() || Some(diag_end) == expected.end)
        });
        let expected_end = expected.iter().position(|expected| {
            Some(diag_end) == expected.end
                && compare_message(diag.message(), &expected.message)
                && diag.level() == expected.level
                && (expected.start.is_none() || Some(diag_start) == expected.start)
        });

        let found_match = match (expected_start, expected_end) {
            (Some(start), Some(end)) => {
                // Found both start and end, success!
                // Make sure to remove the larger index first, so the
                // smaller index remains valid.
                expected.remove(start.max(end));
                if start != end {
                    expected.remove(start.min(end));
                }
                true
            }
            (Some(start), None) => {
                println!("{path:?}: Could not find end of error/warning: {diag:#?}");
                expected.remove(start);
                false
            }
            (None, Some(end)) => {
                println!("{path:?}: Could not find start of error/warning: {diag:#?}");
                expected.remove(end);
                false
            }
            // TODO: Remove start/end if only one was found
            (None, None) => {
                println!("{path:?}: Unexpected error/warning: {diag:#?}, {diag_start}, {diag_end}",);
                false
            }
        };

        if !found_match {
            success = false;

            #[cfg(feature = "display-diagnostics")]
            if !_silent {
                let mut to_report = BuildDiagnostics::default();
                to_report.push_compiler_error((*diag).clone());
                to_report.print();
            }
        }
    }

    for expected in expected {
        success = false;
        println!(
            "{path:?}: {level:?} not found at offset {start:?}-{end:?}: {message:?}",
            level = expected.level,
            start = expected.start,
            end = expected.end,
            message = expected.message
        );
    }

    if !success && update {
        let mut source = source.to_string();
        self::update(&diags, &mut source, lines, &captures);
        std::fs::write(path, source).unwrap();
    }

    Ok(success)
}

/// Rewrite the source to remove the old comments and add accurate error comments
fn update(
    diags: &[&Diagnostic],
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

    let mut last_line_adjust = Vec::from_iter(std::iter::repeat_n(0, lines.len()));

    for d in diags {
        let mut insert_range_at = |range: &str, l, c: usize| {
            let column_adjust = if c < 3 { "<".repeat(3 - c) } else { "".to_string() };
            let byte_offset = lines[l - 1] + 1;
            let level = match d.level() {
                DiagnosticLevel::Error => "error",
                DiagnosticLevel::Warning => "warning",
                DiagnosticLevel::Note => "note",
                _ => todo!(),
            };
            let to_insert = format!(
                "//{indent}{range}{adjust}{column_adjust}{level}{{{message}}}\n",
                indent = " ".repeat(c.max(3) - 3),
                adjust = "^".repeat(last_line_adjust[l - 1]),
                message = d.message().replace('\n', "↵").replace(env!("CARGO_MANIFEST_DIR"), "📂")
            );
            if byte_offset > source.len() {
                source.push('\n');
            }
            source.insert_str(byte_offset, &to_insert);
            for line in (l - 1)..lines.len() {
                lines[line] += to_insert.len();
            }
            last_line_adjust[l - 1] += 1;
        };

        let (line_start, column_start) = d.line_column();
        // end_line_column is not (yet) available via the public API, so use the private API
        // instead.
        let (line_end, column_end) =
            diagnostics::diagnostic_end_line_column_with_format(d, ByteFormat::Utf8);

        // The end column is exclusive, therefore use - 1 here
        let range = if d.length() <= 1 {
            // Single-character diagnostic, use "^" for the marker for 1-character diagnostics,
            // use "|" for 0-character diagnostics
            if d.length() == 0 { "|" } else { "^" }.to_owned()
        } else {
            let end = if line_start == line_end {
                // Same line, we can insert the closing "<"
                " ".repeat(column_end - column_start - 2) + "<"
            } else {
                // End is on a different line, we'll emit it later
                "".to_owned()
            };
            format!(">{end}")
        };

        insert_range_at(&range, line_start, column_start);

        // Insert the closing `<` at another line if necessary
        // Edge-case: If a single-character diagnostic is on a newline character (\n), its
        // end_line_column is technically on a new line, but the single ^ marker is enough, so no
        // closing character is needed.
        if line_start != line_end && d.length() > 1 {
            insert_range_at("<", line_end, column_end - 1);
        }
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
    compiler_config.library_paths = [(
        "test-lib".into(),
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/typeloader/library").into(),
    )]
    .into_iter()
    .collect();
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
//                                   > <error{Syntax error: expected '{'}
    "#
    )?);

    // also when it's shifted up by an additional ^
    assert!(process(
        r#"
export component Foo inherits Window foo { width: 10px; }

//                                   > <^error{Syntax error: expected '{'}
    "#
    )?);

    // also when it's shifted left by additional <
    assert!(process(
        r#"
export component Foo inherits Window foo { width: 10px; }
//                                      > <<<<error{Syntax error: expected '{'}
    "#
    )?);

    // or split into multiple lines
    assert!(process(
        r#"
export component Foo inherits Window foo { width: 10px; }
//                                   >error{Syntax error: expected '{'}
//                                     <^error{Syntax error: expected '{'}
    "#
    )?);

    // But not if it is at the wrong position
    assert!(!process(
        r#"
export component Foo inherits Window foo { width: 10px; }
//                                    > <error{Syntax error: expected '{'}
    "#
    )?);

    // or the wrong line
    assert!(!process(
        r#"
export component Foo inherits Window foo { width: 10px; }

//                                   > <error{Syntax error: expected '{'}
    "#
    )?);

    // or the wrong message
    assert!(!process(
        r#"
export component Foo inherits Window foo { width: 10px; }
//                                   > <error{foo_bar}
    "#
    )?);

    // or the wrong line because two carets
    assert!(!process(
        r#"

export component Foo inherits Window foo { width: 10px; }
//                                   > <^^error{Syntax error: expected '{'}
    "#
    )?);

    // Even on windows, it should work
    assert!(process(
        "\r\n\
export component Foo inherits Window foo { width: 10px; }\r\n\
//                                   > <error{Syntax error: expected '{'}\r\n\
"
    )?);

    Ok(())
}
