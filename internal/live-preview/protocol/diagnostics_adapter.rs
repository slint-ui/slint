// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;

use i_slint_compiler::diagnostics::{self, ByteFormat, Diagnostic, DiagnosticLevel, Spanned};
use lsp_types::Url;

use super::SourceFileVersion;

/// A diagnostic in a source without a URL, such as a relative path in a test, has none.
pub fn diagnostic_url(d: &Diagnostic) -> Option<Url> {
    Spanned::source_file(d)?.path().to_url()
}

/// Sorts a compilation's diagnostics into the files they're in,
/// for [`super::PreviewToLsp::notify_diagnostics()`].
///
/// `files` are the files the compilation read, with the version it read.
/// Each one gets an entry, so an empty list clears what the editor showed for that file.
/// Diagnostics in other files are dropped.
pub fn diagnostics_by_url(
    diagnostics: &[Diagnostic],
    files: impl IntoIterator<Item = (Url, SourceFileVersion)>,
    format: ByteFormat,
) -> HashMap<Url, (SourceFileVersion, Vec<lsp_types::Diagnostic>)> {
    let mut result: HashMap<_, _> =
        files.into_iter().map(|(url, version)| (url, (version, Vec::new()))).collect();
    for diagnostic in diagnostics {
        if let Some((_, file_diagnostics)) =
            diagnostic_url(diagnostic).and_then(|url| result.get_mut(&url))
        {
            file_diagnostics.push(to_lsp_diagnostic(diagnostic, format));
        }
    }
    result
}

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
        DiagnosticLevel::Info => DiagnosticSeverity::INFORMATION,
        DiagnosticLevel::Note => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::INFORMATION,
    }
}
