// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
    Work in progress for a formatter.
    Use like this to format a file:
    ```sh
        cargo run --bin slint-fmt -- -i some_file.slint
    ```

    Some code in this main.rs file is duplicated with the slint-updater, i guess it could
    be refactored in a separate utility crate or module or something.

    The [`writer::TokenWriter`] trait is meant to be able to support the LSP later as the
    LSP wants just the edits, not the full file
*/

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::{syntax_nodes, SyntaxNode};
use std::io::Write;
use std::path::Path;

use clap::Parser;

mod fmt;
mod writer;

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(name = "path to .slint file(s)", action)]
    paths: Vec<std::path::PathBuf>,

    /// modify the file inline instead of printing to stdout
    #[arg(short, long, action)]
    inline: bool,
}

fn main() -> std::io::Result<()> {
    let args = Cli::parse();

    for path in args.paths {
        let source = std::fs::read_to_string(&path)?;

        if args.inline {
            let file = std::fs::File::create(&path)?;
            process_file(source, path, file)?
        } else {
            process_file(source, path, std::io::stdout())?
        }
    }
    Ok(())
}

/// FIXME! this is duplicated with the updater
fn process_rust_file(source: String, mut file: impl Write) -> std::io::Result<()> {
    let mut source_slice = &source[..];
    let slint_macro = format!("{}!", "slint"); // in a variable so it does not appear as is
    'l: while let Some(idx) = source_slice.find(&slint_macro) {
        // Note: this code ignore string literal and unbalanced comment, but that should be good enough
        let idx2 =
            if let Some(idx2) = source_slice[idx..].find(|c| c == '{' || c == '(' || c == '[') {
                idx2
            } else {
                break 'l;
            };
        let open = source_slice.as_bytes()[idx + idx2].into();
        let close = match open {
            '{' => '}',
            '(' => ')',
            '[' => ']',
            _ => panic!(),
        };
        file.write_all(source_slice[..=idx + idx2].as_bytes())?;
        source_slice = &source_slice[idx + idx2 + 1..];
        let mut idx = 0;
        let mut count = 1;
        while count > 0 {
            if let Some(idx2) = source_slice[idx..].find(|c| {
                if c == open {
                    count += 1;
                    true
                } else if c == close {
                    count -= 1;
                    true
                } else {
                    false
                }
            }) {
                idx += idx2 + 1;
            } else {
                break 'l;
            }
        }
        let code = &source_slice[..idx - 1];
        source_slice = &source_slice[idx - 1..];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        visit_node(syntax_node, &mut file)?;
        if diag.has_error() {
            file.write_all(&code.as_bytes()[len..])?;
            diag.print();
        }
    }
    return file.write_all(source_slice.as_bytes());
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
        file.write_all(source_slice[..=code_start - 1].as_bytes())?;
        source_slice = &source_slice[code_start..];
        let code = &source_slice[..code_end];
        source_slice = &source_slice[code_end..];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        visit_node(syntax_node, &mut file)?;
        if diag.has_error() {
            file.write_all(&code.as_bytes()[len..])?;
            diag.print();
        }
    }
    return file.write_all(source_slice.as_bytes());
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
    if diag.has_error() {
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
            // This allows usage like `cat x.slint | slint-fmt /dev/stdin`
            if path.as_path() == Path::new("/dev/stdin") {
                return process_slint_file(source, path, file);
            }
            // With other file types, we just output them in their original form.
            return file.write_all(source.as_bytes());
        }
    }
}

fn visit_node(node: SyntaxNode, file: &mut impl Write) -> std::io::Result<()> {
    if let Some(doc) = syntax_nodes::Document::new(node) {
        let mut writer = writer::FileWriter { file };
        fmt::format_document(doc, &mut writer)
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Not a Document"))
    }
}
