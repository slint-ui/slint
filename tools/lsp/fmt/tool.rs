// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore inplace
/*!
   Command-line entry point for `slint-lsp format`.

   Use like this:
   ```sh
       cargo run --bin slint-lsp -- format -i some_file.slint
   ```

   The embedded Rust/Markdown handling is still duplicated with `slint-updater`.
*/

use i_slint_formatter::Formatter;
use std::io::{BufWriter, Write};
use std::path::Path;

pub fn run(files: &[std::path::PathBuf], inplace: bool) -> std::io::Result<()> {
    let formatter = Formatter::new().map_err(formatter_error_to_io)?;

    for path in files {
        let source = std::fs::read_to_string(path)?;

        if inplace {
            let file = BufWriter::new(std::fs::File::create(path)?);
            process_file(source, path, file, &formatter)?;
        } else {
            process_file(source, path, std::io::stdout(), &formatter)?;
        }
    }
    Ok(())
}

fn formatter_error_to_io(err: i_slint_formatter::FormatError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, err.to_string())
}

/// FIXME! this is duplicated with the updater
fn process_rust_file(
    source: String,
    mut file: impl Write,
    formatter: &Formatter,
) -> std::io::Result<()> {
    let mut last = 0;
    for range in i_slint_compiler::lexer::locate_slint_macro(&source) {
        file.write_all(&source.as_bytes()[last..=range.start])?;
        last = range.end;
        let code = &source[range];
        format_embedded_slint(code, &mut file, formatter)?;
    }
    file.write_all(&source.as_bytes()[last..])?;
    file.flush()
}

/// FIXME! this is duplicated with the updater
fn process_markdown_file(
    source: String,
    mut file: impl Write,
    formatter: &Formatter,
) -> std::io::Result<()> {
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
        format_embedded_slint(code, &mut file, formatter)?;
    }
    file.write_all(source_slice.as_bytes())
}

fn process_slint_file(
    source: String,
    _path: &std::path::Path,
    mut file: impl Write,
    formatter: &Formatter,
) -> std::io::Result<()> {
    let formatted = formatter.format_str(&source).map_err(formatter_error_to_io)?;
    file.write_all(formatted.text.as_bytes())?;
    file.flush()
}

fn process_file(
    source: String,
    path: &std::path::Path,
    file: impl Write,
    formatter: &Formatter,
) -> std::io::Result<()> {
    match path.extension() {
        Some(ext) if ext == "rs" => process_rust_file(source, file, formatter),
        Some(ext) if ext == "md" => process_markdown_file(source, file, formatter),
        // Formatting .60 files because of backwards compatibility (project was recently renamed)
        Some(ext) if ext == "slint" || ext == ".60" => {
            process_slint_file(source, path, file, formatter)
        }
        _ => {
            // This allows usage like `cat x.slint | slint-lsp format /dev/stdin`
            if path == Path::new("/dev/stdin") {
                return process_slint_file(source, path, file, formatter);
            }
            // With other file types, we just output them in their original form.
            let mut file = file;
            file.write_all(source.as_bytes())
        }
    }
}

fn format_embedded_slint(
    source: &str,
    mut file: impl Write,
    formatter: &Formatter,
) -> std::io::Result<()> {
    let formatted = formatter.format_str(source).map_err(formatter_error_to_io)?;
    file.write_all(formatted.text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn process(path: &str, source: &str) -> String {
        let formatter = Formatter::new().expect("formatter should initialize");
        let mut output = Vec::new();
        process_file(source.into(), Path::new(path), &mut output, &formatter)
            .expect("processing should succeed");
        String::from_utf8(output).expect("formatter should emit valid UTF-8")
    }

    #[test]
    fn formats_embedded_slint_in_rust_macros() {
        let output = process(
            "sample.rs",
            "fn main() {\n    slint::slint! { export component Demo {x: 42px;} }\n}\n",
        );

        assert!(output.starts_with("fn main() {\n    slint::slint! {"));
        assert!(output.contains("export component Demo { x: 42px; }\n"));
        assert!(output.contains("export component Demo { x: 42px; }\n}\n"));
        assert!(output.ends_with("}\n"));
    }

    #[test]
    fn formats_embedded_slint_in_markdown_fences() {
        let output = process(
            "README.md",
            "Before\n```slint\nexport component Demo {x: 42px;}\n```\nAfter\n",
        );

        assert!(output.starts_with("Before\n```slint\n"));
        assert!(output.contains("export component Demo { x: 42px; }\n"));
        assert!(output.ends_with("```\nAfter\n"));
    }

    #[test]
    fn leaves_unknown_files_unchanged() {
        let source = "not slint";
        assert_eq!(process("notes.txt", source), source);
    }
}
