// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint_interpreter::{Diagnostic, DiagnosticLevel};

/// This enum describes the level or severity of a diagnostic message produced by the compiler.
#[napi(js_name = "DiagnosticLevel")]
pub enum JsDiagnosticLevel {
    /// The diagnostic found is an error that prevents successful compilation.
    Error,

    /// The diagnostic found is a warning.
    Warning,
}

impl From<DiagnosticLevel> for JsDiagnosticLevel {
    fn from(diagnostic_level: DiagnosticLevel) -> Self {
        match diagnostic_level {
            DiagnosticLevel::Warning => JsDiagnosticLevel::Warning,
            _ => JsDiagnosticLevel::Error,
        }
    }
}

/// This structure represent a diagnostic emitted while compiling .slint code.
///
/// It is basically a message, a level (warning or error), attached to a
/// position in the code.
#[napi(object, js_name = "Diagnostic")]
pub struct JsDiagnostic {
    /// The level for this diagnostic.
    pub level: JsDiagnosticLevel,

    /// Message for this diagnostic.
    pub message: String,

    /// The line number in the .slint source file. The line number starts with 1.
    pub line_number: u32,

    // The column in the .slint source file. The column number starts with 1.
    pub column_number: u32,

    /// The path of the source file where this diagnostic occurred.
    pub file_name: Option<String>,
}

impl From<Diagnostic> for JsDiagnostic {
    fn from(internal_diagnostic: Diagnostic) -> Self {
        let (line_number, column) = internal_diagnostic.line_column();
        Self {
            level: internal_diagnostic.level().into(),
            message: internal_diagnostic.message().into(),
            line_number: line_number as u32,
            column_number: column as u32,
            file_name: internal_diagnostic
                .source_file()
                .and_then(|path| path.to_str())
                .map(|str| str.into()),
        }
    }
}
