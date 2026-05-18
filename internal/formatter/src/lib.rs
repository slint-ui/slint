// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod style_profile;

use std::fs;
use std::io;
use std::path::Path;

pub use style_profile::{
    FileStyleProfile, RepositoryStyleComparison, RepositoryStyleProfile, StyleChoice,
    StyleDecision, StyleDecisionComparison, StyleDecisionKind, StyleDecisionSummary,
    aggregate_repository_profile, collect_standalone_slint_files, compare_repository_profiles,
    format_repository_style_comparison_report, format_repository_style_report, profile_file,
    profile_source, profile_source_with_path,
};
use topiary_core::{Language, Operation, TopiaryQuery};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatResult {
    pub text: String,
    pub changed: bool,
}

#[derive(Debug)]
pub struct Formatter {
    language: Language,
}

#[derive(Debug)]
pub enum FormatError {
    Io(io::Error),
    Formatter(topiary_core::FormatterError),
    UnsupportedPath(std::path::PathBuf),
}

impl Formatter {
    pub fn new() -> Result<Self, FormatError> {
        Ok(Self { language: load_slint_language()? })
    }

    pub fn format_str(&self, source: &str) -> Result<FormatResult, FormatError> {
        let mut output = Vec::new();
        topiary_core::formatter_str(source, &mut output, &self.language, default_operation())?;

        let text = String::from_utf8(output).expect("topiary should emit valid UTF-8");
        Ok(FormatResult { changed: text != source, text })
    }

    pub fn format_path(&self, path: impl AsRef<Path>) -> Result<FormatResult, FormatError> {
        let path = path.as_ref();
        if path.extension().and_then(|extension| extension.to_str()) != Some("slint") {
            return Err(FormatError::UnsupportedPath(path.to_owned()));
        }

        let text = fs::read_to_string(path)?;
        self.format_str(&text)
    }
}

fn load_slint_language() -> Result<Language, FormatError> {
    let grammar = topiary_tree_sitter_facade::Language::from(i_tree_sitter_slint::LANGUAGE);
    let query = TopiaryQuery::new(&grammar, include_str!("slint.scm"))?;

    Ok(Language { name: "slint".into(), query, grammar, indent: Some("    ".into()) })
}

fn default_operation() -> Operation {
    Operation::Format { skip_idempotence: false, tolerate_parsing_errors: false }
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => err.fmt(f),
            Self::Formatter(err) => err.fmt(f),
            Self::UnsupportedPath(path) => {
                write!(f, "only standalone .slint files are supported: {}", path.display())
            }
        }
    }
}

impl std::error::Error for FormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Formatter(err) => Some(err),
            Self::UnsupportedPath(_) => None,
        }
    }
}

impl From<io::Error> for FormatError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<topiary_core::FormatterError> for FormatError {
    fn from(err: topiary_core::FormatterError) -> Self {
        Self::Formatter(err)
    }
}

#[cfg(test)]
mod tests {
    use super::Formatter;

    #[test]
    fn formats_a_standalone_slint_component() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component TestCase inherits Rectangle {\n    x: 42px;\n}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert!(result.changed);
        assert_eq!(result.text, "export component TestCase inherits Rectangle { x: 42px; }\n");
    }
}
