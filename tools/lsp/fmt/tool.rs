// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
    Work in progress for a formatter.
    Use like this to format a file:
    ```sh
        cargo run --bin slint-lsp -- format -i some_file.slint
    ```

    Some code in this main.rs file is duplicated with the slint-updater, i guess it could
    be refactored in a separate utility crate or module or something.

    The [`writer::TokenWriter`] trait is meant to be able to support the LSP later as the
    LSP wants just the edits, not the full file
*/

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::{syntax_nodes, SyntaxNode};
use std::io::{BufWriter, Write};
use std::path::Path;

use super::{fmt, writer};

pub fn run(files: Vec<std::path::PathBuf>, inplace: bool) -> std::io::Result<usize> {
    let mut changed_or_error_files = 0;

    for path in files {
        let source = std::fs::read(&path)?;

        match process_file(source.clone(), &path) {
            Ok(result) if result == source => {}
            Ok(result) => {
                changed_or_error_files += 1;

                if inplace {
                    let mut file = BufWriter::new(std::fs::File::create(&path)?);
                    file.write_all(&result)?;
                } else {
                    std::io::stdout().write_all(&result)?;
                }
            }
            Err(e) => {
                println!("Error in {path:?}: {e}");
                changed_or_error_files += 1;
            }
        }
    }
    Ok(changed_or_error_files)
}

/// FIXME! this is duplicated with the updater
fn process_rust_file(source: Vec<u8>) -> std::io::Result<Vec<u8>> {
    let mut result = String::new();
    let source =
        String::from_utf8(source).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let mut last = 0;
    for range in i_slint_compiler::lexer::locate_slint_macro(&source) {
        result.push_str(&source[last..=range.start]);
        last = range.end;
        let code = &source[range];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        result.push_str(&visit_node(syntax_node)?);

        if diag.has_errors() {
            result.push_str(&code[len..]);
            diag.print();
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from("Failed to parse file"),
            ));
        }
    }
    result.push_str(&source[last..]);
    Ok(result.as_bytes().to_vec())
}

fn find_slint_code_in_markdown(content: &[u8]) -> Option<(usize, usize)> {
    const CODE_FENCE_START: &[u8] = b"```slint\n";
    const CODE_FENCE_END: &[u8] = b"```\n";

    let mut it = content.iter().enumerate();

    let mut fence_offset = 0;

    let mut code_start = usize::MAX;
    let mut code_end = usize::MAX;

    #[allow(clippy::while_let_on_iterator)]
    while let Some((idx, b)) = it.next() {
        if *b == CODE_FENCE_START[fence_offset] {
            fence_offset += 1;
        }

        if fence_offset == CODE_FENCE_START.len() {
            code_start = idx + 1;
            break;
        }
    }

    fence_offset = 0;
    let mut possible_end_offset = 0;

    #[allow(clippy::while_let_on_iterator)]
    while let Some((idx, b)) = it.next() {
        if *b == CODE_FENCE_END[fence_offset] {
            if fence_offset == 0 {
                possible_end_offset = idx - 1;
            }
            fence_offset += 1;
        }

        if fence_offset == CODE_FENCE_END.len() {
            code_end = possible_end_offset;
            break;
        }
    }

    if code_end != usize::MAX {
        Some((code_start, code_end))
    } else {
        None
    }
}

/// FIXME! this is duplicated with the updater
fn process_markdown_file(source: Vec<u8>) -> std::io::Result<Vec<u8>> {
    let mut result = Vec::new();

    let mut source_slice = &source[..];
    while let Some((code_start, code_end)) = find_slint_code_in_markdown(source_slice) {
        result.extend(&source_slice[..code_start]);
        let code = Vec::from(&source_slice[code_start..code_end]);
        let code = String::from_utf8(code)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        source_slice = &source_slice[code_end..];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code, None, &mut diag);

        result.extend(visit_node(syntax_node)?.as_bytes());

        if diag.has_errors() {
            diag.print();
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from("Failed to parse file"),
            ));
        }
    }
    result.extend(source_slice);
    Ok(result)
}

fn process_slint_file(source: Vec<u8>, path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let mut diag = BuildDiagnostics::default();
    let source =
        String::from_utf8(source).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let syntax_node = i_slint_compiler::parser::parse(source, Some(path), &mut diag);

    let result = visit_node(syntax_node)?.as_bytes().to_vec();

    if diag.has_errors() {
        diag.print();
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            String::from("Failed to parse file"),
        ));
    }
    Ok(result)
}

fn process_file(source: Vec<u8>, path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    match path.extension() {
        Some(ext) if ext == "rs" => process_rust_file(source),
        Some(ext) if ext == "md" => process_markdown_file(source),
        // Formatting .60 files because of backwards compatibility (project was recently renamed)
        Some(ext) if ext == "slint" || ext == ".60" => process_slint_file(source, path),
        _ => {
            // This allows usage like `cat x.slint | slint-lsp format -`
            if path == Path::new("/dev/stdin") || path == Path::new("-") {
                return process_slint_file(source, path);
            }

            Ok(source.to_vec())
        }
    }
}

fn visit_node(node: SyntaxNode) -> std::io::Result<String> {
    if let Some(doc) = syntax_nodes::Document::new(node) {
        let mut writer = writer::StringWriter::default();
        fmt::format_document(doc, &mut writer)?;
        Ok(writer.finalize())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Not a Document"))
    }
}
