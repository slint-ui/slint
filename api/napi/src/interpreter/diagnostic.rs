// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial


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

#[napi(js_name = "Diagnostic")]
pub struct JsDiagnostic {
    internal: Diagnostic,
}

#[napi]
impl JsDiagnostic {
    #[napi]
    pub fn level(&self) -> JsDiagnosticLevel {
        self.internal.level().into()
    }

    #[napi]
    pub fn message(&self) -> String {
        self.internal.message().into()
    }

    #[napi]
    pub fn line_column(&self) -> Vec<i32> {
        let (start, column) = self.internal.line_column();
        vec![start as i32, column as i32]
    }

    #[napi]
    pub fn source_file(&self) -> Option<String> {
        if let Some(source_file) = self.internal.source_file() {
            if let Some(source_file) = source_file.to_str() {
                return Some(source_file.into());
            }
        }
        None
    }
}

impl From<Diagnostic> for JsDiagnostic {
    fn from(diagnostic: Diagnostic) -> Self {
        Self { internal: diagnostic }
    }
}
