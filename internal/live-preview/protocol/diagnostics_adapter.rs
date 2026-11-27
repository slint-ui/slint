// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::BTreeMap;

use i_slint_compiler::diagnostics::{
    self, ByteFormat, Diagnostic, DiagnosticLevel, Spanned, Suggestion,
};
use lsp_types::Url;

use super::SourceFileVersion;

/// A diagnostic in a source without a URL, such as a relative path in a test, has none.
pub fn diagnostic_url(d: &Diagnostic) -> Option<Url> {
    Spanned::source_file(d)?.path().to_url()
}

/// Sorts a compilation's diagnostics into the files they're in,
/// for [`super::PreviewToLsp::notify_diagnostics()`].
///
/// `files` are the files with a known version.
/// Each one gets an entry, so an empty list clears what the editor showed for that file.
/// A diagnostic in another file goes under that file without a version.
pub fn diagnostics_by_url(
    diagnostics: &[Diagnostic],
    files: impl IntoIterator<Item = (Url, SourceFileVersion)>,
    format: ByteFormat,
) -> BTreeMap<Url, (SourceFileVersion, Vec<lsp_types::Diagnostic>)> {
    let mut result: BTreeMap<_, _> =
        files.into_iter().map(|(url, version)| (url, (version, Vec::new()))).collect();
    for diagnostic in diagnostics {
        if let Some(url) = diagnostic_url(diagnostic) {
            result.entry(url).or_default().1.extend(to_lsp_diagnostics(diagnostic, format));
        }
    }
    result
}

/// `d` as LSP diagnostics: `d` itself, followed by its help as a hint.
pub fn to_lsp_diagnostics(
    d: &Diagnostic,
    format: ByteFormat,
) -> impl Iterator<Item = lsp_types::Diagnostic> {
    let result = to_lsp_diagnostic(d, format);
    let help = diagnostics::diagnostic_help(d).map(|help| lsp_types::Diagnostic {
        severity: Some(lsp_types::DiagnosticSeverity::HINT),
        message: help.message.clone(),
        data: None,
        ..result.clone()
    });
    std::iter::once(result).chain(help)
}

/// A suggestion of a diagnostic, in the `data` of the LSP diagnostic.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LspSuggestion {
    pub edit: lsp_types::TextEdit,
    /// The text that `edit` replaces, which the document may have changed since.
    pub replaced: String,
}

/// `d` as an LSP diagnostic, without its help.
/// Its `data` holds the help's suggestions, as an array of [`LspSuggestion`].
pub fn to_lsp_diagnostic(d: &Diagnostic, format: ByteFormat) -> lsp_types::Diagnostic {
    let start = diagnostics::diagnostic_line_column_with_format(d, format);
    let end = diagnostics::diagnostic_end_line_column_with_format(d, format);
    let mut result = lsp_types::Diagnostic::new(
        to_range(start, end),
        Some(to_severity(d.level())),
        None,
        None,
        diagnostics::diagnostic_message(d).to_owned(),
        None,
        None,
    );
    if let Some(sf) = Spanned::source_file(d)
        && let Some(help) = diagnostics::diagnostic_help(d)
        && !help.suggestions.is_empty()
    {
        let range = |s: &Suggestion| s.span.offset..s.span.offset + s.span.length;
        let suggestions = help
            .suggestions
            .iter()
            .map(|s| {
                let start = sf.line_column(s.span.offset, format);
                let end = sf.line_column(range(s).end, format);
                LspSuggestion {
                    edit: lsp_types::TextEdit::new(to_range(start, end), s.replacement.clone()),
                    replaced: sf.source().and_then(|t| t.get(range(s))).unwrap_or_default().into(),
                }
            })
            .collect::<Vec<_>>();
        result.data = serde_json::to_value(suggestions).ok();
    }
    result
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

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_compiler::diagnostics::{
        BuildDiagnostics, Help, SourceFileInner, SourceLocation, Span, Suggestion,
    };
    use i_slint_compiler::source_path::SourcePath;

    /// Used to drop a diagnostic in a file that wasn't listed.
    #[test]
    fn a_file_without_a_version_keeps_its_diagnostics() {
        let listed = Url::from_file_path(std::env::temp_dir().join("listed.slint")).unwrap();
        let unlisted = Url::from_file_path(std::env::temp_dir().join("unlisted.slint")).unwrap();
        let mut build_diagnostics = BuildDiagnostics::default();
        build_diagnostics.push_error_with_span(
            "error".into(),
            SourceLocation {
                source_file: Some(std::sync::Arc::new(SourceFileInner::new(
                    SourcePath::from_url(&unlisted),
                    "error".into(),
                ))),
                span: Span::new(0, 5),
            },
        );
        let diagnostics: Vec<_> = build_diagnostics.into_iter().collect();

        let result =
            diagnostics_by_url(&diagnostics, [(listed.clone(), Some(3))], ByteFormat::Utf8);

        assert_eq!(result[&listed], (Some(3), Vec::new()));
        let (version, unlisted_diagnostics) = &result[&unlisted];
        assert_eq!(*version, None);
        assert_eq!(unlisted_diagnostics.len(), 1, "{unlisted_diagnostics:?}");
    }

    #[test]
    fn help_is_a_hint_after_the_diagnostic() {
        let source_file = std::sync::Arc::new(SourceFileInner::new(
            SourcePath::new("test.slint"),
            "with: 10px;".into(),
        ));
        let location = SourceLocation { source_file: Some(source_file), span: Span::new(0, 4) };
        let mut build_diagnostics = BuildDiagnostics::default();
        build_diagnostics.push_error_with_help(
            "Unknown property with".into(),
            &location,
            Help::new("Did you mean 'width'?", [Suggestion::replace(&location, "width")]),
        );
        let diagnostic = build_diagnostics.into_iter().next().unwrap();

        let result: Vec<_> = to_lsp_diagnostics(&diagnostic, ByteFormat::Utf8).collect();

        let range =
            lsp_types::Range::new(lsp_types::Position::new(0, 0), lsp_types::Position::new(0, 4));
        let [error, hint] = result.as_slice() else { panic!("{result:?}") };
        assert_eq!(error.message, "Unknown property with");
        assert_eq!(error.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
        assert_eq!(
            error.data,
            serde_json::to_value([LspSuggestion {
                edit: lsp_types::TextEdit::new(range, "width".into()),
                replaced: "with".into(),
            }])
            .ok()
        );
        assert_eq!(hint.message, "Did you mean 'width'?");
        assert_eq!(hint.severity, Some(lsp_types::DiagnosticSeverity::HINT));
        assert_eq!(hint.range, range);
        assert_eq!(hint.data, None);
    }
}
