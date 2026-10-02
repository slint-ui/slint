// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::diagnostics::{self, ByteFormat, Diagnostic, DiagnosticLevel, Spanned};

/// A diagnostic in a source without a URL, such as a relative path in a test, has none.
pub fn diagnostic_url(d: &Diagnostic) -> Option<lsp_types::Url> {
    Spanned::source_file(d)?.path().to_url()
}

pub fn to_lsp_diagnostic(d: &Diagnostic, format: ByteFormat) -> lsp_types::Diagnostic {
    let (start, end) = if d.line_column() == (0, 0) {
        ((0, 0), (0, 0))
    } else {
        (
            diagnostics::diagnostic_line_column_with_format(d, format),
            diagnostics::diagnostic_end_line_column_with_format(d, format),
        )
    };
    lsp_types::Diagnostic::new(
        to_range(start, end),
        Some(to_severity(d.level())),
        None,
        None,
        d.message().to_owned(),
        None,
        None,
    )
}

fn to_range(start: (usize, usize), end: (usize, usize)) -> lsp_types::Range {
    let start = lsp_types::Position::new(
        (start.0 as u32).saturating_sub(1),
        (start.1 as u32).saturating_sub(1),
    );
    let end = lsp_types::Position::new(
        (end.0 as u32).saturating_sub(1),
        (end.1 as u32).saturating_sub(1),
    );
    lsp_types::Range::new(start, end)
}

fn to_severity(level: DiagnosticLevel) -> lsp_types::DiagnosticSeverity {
    use lsp_types::DiagnosticSeverity;
    match level {
        DiagnosticLevel::Error => DiagnosticSeverity::ERROR,
        DiagnosticLevel::Warning => DiagnosticSeverity::WARNING,
        DiagnosticLevel::Note => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::INFORMATION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_only_diagnostics_have_an_empty_range() {
        let mut diagnostics = diagnostics::BuildDiagnostics::default();
        diagnostics.push_error_with_span(
            "Could not load this file".into(),
            diagnostics::SourceLocation {
                source_file: Some(diagnostics::SourceFileInner::from_path_only(
                    "missing.slint".into(),
                )),
                ..Default::default()
            },
        );
        let diagnostic = diagnostics.into_iter().next().unwrap();
        for format in [ByteFormat::Utf8, ByteFormat::Utf16] {
            let converted = to_lsp_diagnostic(&diagnostic, format);
            assert_eq!(converted.range, lsp_types::Range::default());
            assert_eq!(converted.message, "Could not load this file");
            assert_eq!(converted.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
        }
    }
}
