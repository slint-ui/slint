// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::diagnostics::{self, ByteFormat, Diagnostic, DiagnosticLevel};

pub fn to_lsp_diagnostic(d: &Diagnostic, format: ByteFormat) -> lsp_types::Diagnostic {
    let start = diagnostics::diagnostic_line_column_with_format(d, format);
    let end = diagnostics::diagnostic_end_line_column_with_format(d, format);
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
