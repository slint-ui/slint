// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::fmt::Write as _;
use std::path::Path;

use i_slint_compiler::diagnostics::{BuildDiagnostics, Diagnostic, DiagnosticLevel};
use i_slint_compiler::parser;

const INLINE_INPUT_PATH: &str = "<input>.slint";

pub(super) fn compiler_diagnostics_for_broken_input(
    source: &str,
    path: Option<&Path>,
) -> Option<String> {
    let diagnostics = parse_source(source, path);
    diagnostics.has_errors().then(|| render_diagnostics(source, path, &diagnostics))
}

pub(super) fn source_has_parse_errors(source: &str) -> bool {
    parse_source(source, None).has_errors()
}

fn parse_source(source: &str, path: Option<&Path>) -> BuildDiagnostics {
    let mut diagnostics = BuildDiagnostics::default();
    let source_path = path.or_else(|| Some(Path::new(INLINE_INPUT_PATH)));
    parser::parse(source.to_owned(), source_path, &mut diagnostics);
    diagnostics
}

fn render_diagnostics(
    source: &str,
    fallback_path: Option<&Path>,
    diagnostics: &BuildDiagnostics,
) -> String {
    let fallback_path = fallback_path.unwrap_or_else(|| Path::new(INLINE_INPUT_PATH));
    let mut rendered = String::new();

    for diagnostic in diagnostics.iter() {
        if !rendered.is_empty() {
            rendered.push('\n');
        }
        render_diagnostic(&mut rendered, source, fallback_path, diagnostic)
            .expect("writing to a string should not fail");
    }

    rendered
}

fn render_diagnostic(
    rendered: &mut String,
    source: &str,
    fallback_path: &Path,
    diagnostic: &Diagnostic,
) -> std::fmt::Result {
    let level = match diagnostic.level() {
        DiagnosticLevel::Error => "error",
        DiagnosticLevel::Warning => "warning",
        DiagnosticLevel::Note => "note",
        _ => "diagnostic",
    };
    writeln!(rendered, "{level}: {}", diagnostic.message())?;

    let (line, column) = diagnostic.line_column();
    if line == 0 || column == 0 {
        return Ok(());
    }

    let path = diagnostic
        .source_file()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(fallback_path);
    writeln!(rendered, " --> {}:{line}:{column}", path.display())?;

    let Some(source_line) = source_line(source, line) else {
        return Ok(());
    };

    let gutter_width = line.to_string().len();
    writeln!(rendered, "{line:>gutter_width$} | {source_line}")?;

    let (marker_padding, marker_width) =
        marker_span(source_line, column, diagnostic.length().max(1));
    writeln!(
        rendered,
        "{} | {}{}",
        " ".repeat(gutter_width),
        " ".repeat(marker_padding),
        "^".repeat(marker_width.max(1))
    )
}

fn source_line(source: &str, line: usize) -> Option<&str> {
    source
        .split('\n')
        .nth(line.saturating_sub(1))
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

fn marker_span(source_line: &str, column: usize, length: usize) -> (usize, usize) {
    let start =
        previous_char_boundary(source_line, column.saturating_sub(1).min(source_line.len()));
    let end = previous_char_boundary(source_line, (start + length).min(source_line.len()));
    let padding = source_line[..start].chars().count();
    let width = source_line[start..end].chars().count();
    (padding, width.max(1))
}

fn previous_char_boundary(source: &str, mut index: usize) -> usize {
    while index > 0 && !source.is_char_boundary(index) {
        index -= 1;
    }
    index
}
