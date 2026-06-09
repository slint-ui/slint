// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Diagnostic output for `slint-viewer`: human (stderr) or JSON (stdout).

use i_slint_compiler::diagnostics::{
    ByteFormat, DiagnosticLevel, diagnostic_end_line_column_with_format,
};
use serde::Serialize;
use slint_interpreter::Diagnostic;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, clap::ValueEnum)]
pub enum DiagnosticsFormat {
    /// Colored, human-readable output on stderr.
    #[default]
    Human,
    /// One JSON array of diagnostic objects on stdout.
    Json,
}

/// Print `result`'s diagnostics in the chosen format.
pub fn print_diagnostics(result: &slint_interpreter::CompilationResult, format: DiagnosticsFormat) {
    match format {
        DiagnosticsFormat::Human => result.print_diagnostics(),
        DiagnosticsFormat::Json => {
            let diagnostics: Vec<Diagnostic> = result.diagnostics().collect();
            println!("{}", format_diagnostics_json(&diagnostics));
        }
    }
}

#[derive(Serialize)]
struct JsonDiagnostic<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<std::borrow::Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_column: Option<usize>,
    level: &'static str,
    message: &'a str,
}

fn level_str(level: DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => "error",
        DiagnosticLevel::Warning => "warning",
        DiagnosticLevel::Note => "note",
        _ => "unknown",
    }
}

/// `(0, 0)` is the sentinel for a missing or invalid span.
fn valid_position(line: usize, column: usize) -> (Option<usize>, Option<usize>) {
    if line == 0 { (None, None) } else { (Some(line), Some(column)) }
}

pub fn format_diagnostics_json(diagnostics: &[Diagnostic]) -> String {
    let entries: Vec<JsonDiagnostic<'_>> = diagnostics
        .iter()
        .map(|d| {
            let (start_line, start_col) = d.line_column();
            let (end_line_raw, end_col_raw) =
                diagnostic_end_line_column_with_format(d, ByteFormat::Utf8);
            let (line, column) = valid_position(start_line, start_col);
            let (end_line, end_column) = valid_position(end_line_raw, end_col_raw);
            JsonDiagnostic {
                file: d.source_file().map(|p| p.to_string_lossy()),
                line,
                column,
                end_line,
                end_column,
                level: level_str(d.level()),
                message: d.message(),
            }
        })
        .collect();
    serde_json::to_string_pretty(&entries).expect("serializing diagnostics")
}
