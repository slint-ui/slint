use i_slint_compiler::diagnostics::ByteFormat;
use slint_interpreter::{Diagnostic, DiagnosticLevel};

pub fn to_lsp_diag(d: &Diagnostic, format: ByteFormat) -> lsp_types::Diagnostic {
    use i_slint_compiler::diagnostics;
    let start_line_column = diagnostics::diagnostic_line_column_with_format(d, format);
    let end_line_column = diagnostics::diagnostic_end_line_column_with_format(d, format);
    lsp_types::Diagnostic::new(
        to_range(start_line_column, end_line_column),
        Some(to_lsp_diag_level(d.level())),
        None,
        None,
        d.message().to_owned(),
        None,
        None,
    )
}

/// Convert line-column pairs to an LSP range.
///
/// The start and end are tuples of 1-indexed line-column values.
/// The end must be exclusive.
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

fn to_lsp_diag_level(level: DiagnosticLevel) -> lsp_types::DiagnosticSeverity {
    use lsp_types::DiagnosticSeverity;
    match level {
        DiagnosticLevel::Error => DiagnosticSeverity::ERROR,
        DiagnosticLevel::Warning => DiagnosticSeverity::WARNING,
        DiagnosticLevel::Note => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::INFORMATION,
    }
}
