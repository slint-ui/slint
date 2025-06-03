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

pub fn run(files: Vec<std::path::PathBuf>, inplace: bool) -> std::io::Result<()> {
    for path in files {
        let source = std::fs::read_to_string(&path)?;

        if inplace {
            let file = BufWriter::new(std::fs::File::create(&path)?);
            process_file(source, path, file)?
        } else {
            process_file(source, path, std::io::stdout())?
        }
    }
    Ok(())
}

/// FIXME! this is duplicated with the updater
fn process_rust_file(source: String, mut file: impl Write) -> std::io::Result<()> {
    let mut last = 0;
    for range in i_slint_compiler::lexer::locate_slint_macro(&source) {
        file.write_all(&source.as_bytes()[last..=range.start])?;
        last = range.end;
        let code = &source[range];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        visit_node(syntax_node, &mut file)?;
        if diag.has_errors() {
            file.write_all(&code.as_bytes()[len..])?;
            diag.print();
        }
    }
    file.write_all(&source.as_bytes()[last..])
}

/// FIXME! this is duplicated with the updater
fn process_markdown_file(source: String, mut file: impl Write) -> std::io::Result<()> {
    let mut source_slice = &source[..];
    const CODE_FENCE_START: &str = "```slint\n";
    const CODE_FENCE_END: &str = "```\n";
    'l: while let Some(code_start) =
        source_slice.find(CODE_FENCE_START).map(|idx| idx + CODE_FENCE_START.len())
    {
        let code_end = if let Some(code_end) = source_slice[code_start..].find(CODE_FENCE_END) {
            code_end
        } else {
            break 'l;
        };
        file.write_all(&source_slice.as_bytes()[..=code_start - 1])?;
        source_slice = &source_slice[code_start..];
        let code = &source_slice[..code_end];
        source_slice = &source_slice[code_end..];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        visit_node(syntax_node, &mut file)?;
        if diag.has_errors() {
            file.write_all(&code.as_bytes()[len..])?;
            diag.print();
        }
    }
    file.write_all(source_slice.as_bytes())
}

fn process_slint_file(
    source: String,
    path: std::path::PathBuf,
    mut file: impl Write,
) -> std::io::Result<()> {
    let mut diag = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse(source.clone(), Some(&path), &mut diag);
    let len = syntax_node.node.text_range().end().into();
    visit_node(syntax_node, &mut file)?;
    if diag.has_errors() {
        file.write_all(&source.as_bytes()[len..])?;
        diag.print();
    }
    Ok(())
}

fn process_file(
    source: String,
    path: std::path::PathBuf,
    mut file: impl Write,
) -> std::io::Result<()> {
    match path.extension() {
        Some(ext) if ext == "rs" => process_rust_file(source, file),
        Some(ext) if ext == "md" => process_markdown_file(source, file),
        // Formatting .60 files because of backwards compatibility (project was recently renamed)
        Some(ext) if ext == "slint" || ext == ".60" => process_slint_file(source, path, file),
        _ => {
            // This allows usage like `cat x.slint | slint-lsp format /dev/stdin`
            if path.as_path() == Path::new("/dev/stdin") {
                return process_slint_file(source, path, file);
            }
            // With other file types, we just output them in their original form.
            file.write_all(source.as_bytes())
        }
    }
}

fn visit_node(node: SyntaxNode, file: &mut impl Write) -> std::io::Result<()> {
    if let Some(doc) = syntax_nodes::Document::new(node) {
        let mut writer = writer::FileWriter { file };
        fmt::format_document(doc, &mut writer)
    } else {
        Err(std::io::Error::other("Not a Document"))
    }
}
