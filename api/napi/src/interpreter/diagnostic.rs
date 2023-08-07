// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use napi::bindgen_prelude::{FromNapiValue, ToNapiValue};
use slint_interpreter::{Diagnostic, DiagnosticLevel};

#[napi(js_name = "DiagnosticLevel")]
pub enum JsDiagnosticLevel {
    Error,
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

#[napi(object, js_name = "Diagnostic")]
pub struct JsDiagnostic {
    pub level: JsDiagnosticLevel,
    pub message: String,
    pub line_number: u32,
    pub column: u32,
    pub source_file: Option<String>,
}

impl From<Diagnostic> for JsDiagnostic {
    fn from(internal_diagnostic: Diagnostic) -> Self {
        let (line_number, column) = internal_diagnostic.line_column();
        Self {
            level: internal_diagnostic.level().into(),
            message: internal_diagnostic.message().into(),
            line_number: line_number as u32,
            column: column as u32,
            source_file: internal_diagnostic
                .source_file()
                .and_then(|path| path.to_str())
                .map(|str| str.into()),
        }
    }
}
