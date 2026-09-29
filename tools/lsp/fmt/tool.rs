// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore inplace
/*!
    Work in progress for a formatter.
    Use like this to format a file:
    ```sh
        cargo run --bin slint-lsp -- format -i some_file.slint
    ```

    The [`writer::TokenWriter`] trait is meant to be able to support the LSP later as the
    LSP wants just the edits, not the full file
*/

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::syntax_nodes;
use std::io::{BufWriter, Write};
use std::path::Path;

use super::{fmt, writer};

pub fn run(files: &[std::path::PathBuf], inplace: bool) -> std::io::Result<()> {
    for path in files {
        let source = std::fs::read_to_string(path)?;

        if inplace {
            let file = BufWriter::new(std::fs::File::create(path)?);
            process_file(source, path, file)?;
        } else {
            process_file(source, path, std::io::stdout())?
        }
    }
    Ok(())
}

fn process_slint_code(
    code: &str,
    file: &mut impl Write,
    path: Option<&std::path::Path>,
) -> Result<bool, std::io::Error> {
    let mut diag = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), path, &mut diag);
    let len = syntax_node.text_range().end().into();
    if let Some(doc) = syntax_nodes::Document::new(syntax_node) {
        let mut writer = writer::FileWriter { file };
        fmt::format_document(doc, &mut writer)?;
    } else {
        return Err(std::io::Error::other("Not a Document"));
    }
    if diag.has_errors() {
        match file.write_all(&code.as_bytes()[len..]) {
            Ok(()) => {
                diag.print();
                Ok(true)
            }
            Err(e) => Err(e),
        }
    } else {
        Ok(false)
    }
}

fn process_rust_file(source: String, mut file: impl Write) -> std::io::Result<()> {
    let mut last = 0;
    let mut had_error = false;
    for range in i_slint_compiler::lexer::locate_slint_macro(&source) {
        file.write_all(&source.as_bytes()[last..=range.start])?;
        last = range.end;
        let code = &source[range];
        had_error |= process_slint_code(code, &mut file, None)?;
    }
    file.write_all(&source.as_bytes()[last..])?;
    file.flush()?;
    check_error(had_error)
}

fn process_markdown_file(source: String, mut file: impl Write) -> std::io::Result<()> {
    let mut source_slice = &source[..];
    let mut had_error = false;
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

        had_error |= process_slint_code(code, &mut file, None)?;
    }
    file.write_all(source_slice.as_bytes())?;
    check_error(had_error)
}

fn process_slint_file(
    source: String,
    path: &std::path::Path,
    mut file: impl Write,
) -> std::io::Result<()> {
    check_error(process_slint_code(source.as_str(), &mut file, Some(path))?)
}

fn check_error(error: bool) -> std::io::Result<()> {
    if error {
        Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Parsing failed."))
    } else {
        Ok(())
    }
}

fn process_file(
    source: String,
    path: &std::path::Path,
    mut file: impl Write,
) -> std::io::Result<()> {
    let result = match path.extension() {
        Some(ext) if ext == "rs" => process_rust_file(source, file),
        Some(ext) if ext == "md" => process_markdown_file(source, file),
        Some(ext) if ext == "slint" => process_slint_file(source, path, file),
        _ => {
            // This allows usage like `cat x.slint | slint-lsp format /dev/stdin`
            if path == Path::new("/dev/stdin") {
                return process_slint_file(source, path, file);
            }
            // With other file types, we just output them in their original form.
            file.write_all(source.as_bytes())
        }
    };
    match result {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Formatting {} failed", path.display()),
        )),
        Err(e) => Err(e),
    }
}
