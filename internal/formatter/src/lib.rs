// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub mod profiling;
mod style_profile;

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::{self, SyntaxKind, SyntaxNode, TextSize};
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

#[derive(Clone, Copy)]
enum DelimiterKind {
    Open,
    Close,
}

#[derive(Clone, Copy)]
struct StructuralDelimiter {
    offset: usize,
    kind: DelimiterKind,
}

impl Formatter {
    pub fn new() -> Result<Self, FormatError> {
        Ok(Self { language: slint_language()? })
    }

    pub fn format_str(&self, source: &str) -> Result<FormatResult, FormatError> {
        let text = self.try_safe_format(source).map_or_else(
            || normalize_layout(source).unwrap_or_else(|| source.to_owned()),
            |text| normalize_layout(&text).unwrap_or(text),
        );
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

pub fn slint_language() -> Result<Language, FormatError> {
    let grammar = topiary_tree_sitter_facade::Language::from(i_tree_sitter_slint::LANGUAGE);
    let query = TopiaryQuery::new(&grammar, include_str!("slint.scm"))?;

    Ok(Language { name: "slint".into(), query, grammar, indent: Some("    ".into()) })
}

fn default_operation() -> Operation {
    Operation::Format { skip_idempotence: false, tolerate_parsing_errors: false }
}

fn tolerant_operation() -> Operation {
    Operation::Format { skip_idempotence: true, tolerate_parsing_errors: true }
}

impl Formatter {
    fn try_safe_format(&self, source: &str) -> Option<String> {
        match self.format_with_operation(source, default_operation()) {
            Ok(text) if self.output_parses(&text) => self
                .try_stable_output(&text, default_operation())
                .or_else(|| self.try_safe_fallback(source)),
            Ok(_) | Err(_) => self.try_safe_fallback(source),
        }
    }

    fn try_safe_fallback(&self, source: &str) -> Option<String> {
        let text = self.format_with_operation(source, tolerant_operation()).ok()?;
        self.try_stable_output(&text, tolerant_operation())
    }

    fn format_with_operation(
        &self,
        source: &str,
        operation: Operation,
    ) -> Result<String, FormatError> {
        let mut output = Vec::new();
        topiary_core::formatter_str(source, &mut output, &self.language, operation)?;
        Ok(String::from_utf8(output).expect("topiary should emit valid UTF-8"))
    }

    fn output_parses(&self, source: &str) -> bool {
        !profile_source(source).has_parse_errors
    }

    fn try_stable_output(&self, source: &str, operation: Operation) -> Option<String> {
        let source = normalize_layout(source)?;
        if !self.output_parses(&source) {
            return None;
        }

        let second_pass = self.format_with_operation(&source, operation).ok()?;
        let second_pass = normalize_layout(&second_pass)?;
        (second_pass == source).then_some(source)
    }
}

fn normalize_layout(source: &str) -> Option<String> {
    normalize_indentation(source)
}

fn normalize_indentation(source: &str) -> Option<String> {
    let document = parse_document(source)?;
    let delimiters = structural_delimiters(&document);
    if delimiters.is_empty() {
        return Some(source.to_owned());
    }

    let mut output = String::with_capacity(source.len());
    let mut depth = 0usize;
    let mut delimiter_index = 0usize;
    let mut offset = 0usize;

    for line in source.split_inclusive('\n') {
        let line_end = offset + line.len();
        let (without_ending, ending) = split_line_ending(line);
        let trimmed = without_ending.trim_start_matches(|ch: char| ch.is_whitespace());

        if trimmed.is_empty() {
            output.push_str(ending);
        } else {
            let leading_closes = trimmed.chars().take_while(|ch| matches!(ch, '}' | ']')).count();
            let indent_level = depth.saturating_sub(leading_closes);
            for _ in 0..indent_level {
                output.push_str("    ");
            }
            output.push_str(trimmed);
            output.push_str(ending);
        }

        while delimiter_index < delimiters.len() && delimiters[delimiter_index].offset < line_end {
            match delimiters[delimiter_index].kind {
                DelimiterKind::Open => depth += 1,
                DelimiterKind::Close => depth = depth.saturating_sub(1),
            }
            delimiter_index += 1;
        }

        offset = line_end;
    }

    Some(output)
}

fn parse_document(source: &str) -> Option<SyntaxNode> {
    let mut diagnostics = BuildDiagnostics::default();
    let document = parser::parse(source.to_owned(), None, &mut diagnostics);
    (!diagnostics.has_errors()).then_some(document)
}

fn structural_delimiters(document: &SyntaxNode) -> Vec<StructuralDelimiter> {
    let Some(mut token) = document.first_token() else {
        return Vec::new();
    };

    let end = byte_offset(document.text_range().end());
    let mut delimiters = Vec::new();

    loop {
        if byte_offset(token.text_range().start()) >= end {
            break;
        }

        let kind = match token.kind() {
            SyntaxKind::LBrace | SyntaxKind::LBracket => Some(DelimiterKind::Open),
            SyntaxKind::RBrace | SyntaxKind::RBracket => Some(DelimiterKind::Close),
            _ => None,
        };
        if let Some(kind) = kind {
            delimiters.push(StructuralDelimiter {
                offset: byte_offset(token.text_range().start()),
                kind,
            });
        }

        let Some(next) = token.next_token() else {
            break;
        };
        token = next;
    }

    delimiters
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(stripped) = line.strip_suffix('\n') {
        if let Some(stripped) = stripped.strip_suffix('\r') {
            (stripped, "\r\n")
        } else {
            (stripped, "\n")
        }
    } else {
        (line, "")
    }
}

fn byte_offset(text_size: TextSize) -> usize {
    u32::from(text_size) as usize
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
    use super::{Formatter, default_operation, profile_source};

    #[test]
    fn formats_a_standalone_slint_component() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component TestCase inherits Rectangle {\n    x: 42px;\n}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert!(result.changed);
        assert_eq!(
            result.text,
            "export component TestCase inherits Rectangle {\n    x: 42px;\n}\n"
        );
    }

    #[test]
    fn keeps_simple_callback_blocks_inline() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component TestCase inherits Rectangle {\n    clicked=>{ foo(); }\n}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "export component TestCase inherits Rectangle {\n    clicked => { foo(); }\n}\n"
        );
    }

    #[test]
    fn formats_multiline_block_with_two_sibling_statements() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component TestCase inherits Rectangle {\n    x: 42px;\n    y: 12px;\n}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "export component TestCase inherits Rectangle {\n    x: 42px;\n    y: 12px;\n}\n"
        );
    }

    #[test]
    fn formats_nested_multiline_block_with_two_sibling_elements() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component TestCase inherits Rectangle {\n    GridLayout {\n        a := Rectangle {\n            x: 1px;\n            y: 2px;\n        }\n        b := Rectangle {\n            x: 3px;\n            y: 4px;\n        }\n    }\n}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "export component TestCase inherits Rectangle {\n    GridLayout {\n        a := Rectangle {\n            x: 1px;\n            y: 2px;\n        }\n        b := Rectangle {\n            x: 3px;\n            y: 4px;\n        }\n    }\n}\n"
        );
    }

    #[test]
    fn keeps_color_example_block_indentation_stable() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = include_str!("../../../tests/cases/examples/color.slint");
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(result.text, input);
    }

    #[test]
    fn separates_top_level_definitions_with_a_blank_line() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component First inherits Rectangle {}\ncomponent Second inherits Rectangle {}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "component First inherits Rectangle {}\n\ncomponent Second inherits Rectangle {}\n"
        );
    }

    #[test]
    fn formatted_output_has_no_trailing_spaces() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component First inherits Rectangle {}\ncomponent Second inherits Rectangle {}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert!(result.text.lines().all(|line| !line.ends_with(' ')));
    }

    #[test]
    fn keeps_unit_suffixed_literals_intact() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component TestCase inherits Rectangle { property <color> base: #00007F; background: base.brighter(50%); width: 50%; height: 12px; }";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "export component TestCase inherits Rectangle { property <color> base: #00007F; background: base.brighter(50%); width: 50%; height: 12px; }\n"
        );
    }

    #[test]
    fn formats_indexed_for_loops_without_breaking_parsing() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component TestCase { in property <[int]> values: [1, 2]; for value[idx] in values : Rectangle { x: values[idx]; } }";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert!(result.text.contains("for value[idx] in values: Rectangle { x: values[idx]; }"));
        assert!(!profile_source(&result.text).has_parse_errors);
    }

    #[test]
    fn preserves_blank_lines_around_states_blocks() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component TestCase {\n    callback key-pressed(/* key */ string);\n\n    states [\n        pressed when touch.pressed : {\n            opacity: 0.5;\n        }\n    ]\n}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "component TestCase {\n    callback key-pressed(/* key */ string);\n\n    states [\n        pressed when touch.pressed: {\n            opacity: 0.5;\n        }\n    ]\n}\n"
        );
        assert!(!profile_source(&result.text).has_parse_errors);
    }

    #[test]
    fn formats_multi_target_animate_targets_with_hyphenated_properties() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component TestCase { animate background, border-color { duration: 120ms; } }";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert!(result.changed);
        assert_eq!(
            result.text,
            "component TestCase { animate background, border-color { duration: 120ms; } }\n"
        );
        assert!(!profile_source(&result.text).has_parse_errors);
    }

    #[test]
    fn normalizes_fallback_indentation_for_argument_callbacks() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component X {\n    Slider {\n        changed(d) => { foo(); }\n        minimum: 4;\n        maximum: 5;\n    }\n}\n";
        let perturbed = "component X {\n    Slider {\n          changed(d) => { foo(); }\n          minimum: 4;\n          maximum: 5;\n    }\n}\n";

        let formatted = formatter.format_str(input).expect("formatting should succeed").text;
        let perturbed_formatted =
            formatter.format_str(perturbed).expect("formatting should succeed").text;
        let second_pass = formatter.format_str(&formatted).expect("formatting should succeed").text;

        assert_eq!(perturbed_formatted, formatted);
        assert_eq!(second_pass, formatted);
        assert!(!profile_source(&formatted).has_parse_errors);
    }

    #[test]
    fn normalizes_strict_output_indentation_for_global_blocks() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export global Api {\n    pure callback make-data(task: TaskData, source-column: int, source-index: int) -> data-transfer;\n    // Returns the source column for a payload produced by `make-data`, or -1 for foreign payloads.\n    pure callback source-column-of(data: data-transfer) -> int;\n    callback add-task(data: data-transfer, target-column: int, target-index: int);\n}\n";
        let perturbed = "export global Api {\n      pure callback make-data(task: TaskData, source-column: int, source-index: int) -> data-transfer;\n      // Returns the source column for a payload produced by `make-data`, or -1 for foreign payloads.\n      pure callback source-column-of(data: data-transfer) -> int;\n      callback add-task(data: data-transfer, target-column: int, target-index: int);\n  }\n";

        let formatted = formatter.format_str(input).expect("formatting should succeed").text;
        let perturbed_formatted =
            formatter.format_str(perturbed).expect("formatting should succeed").text;
        let second_pass = formatter.format_str(&formatted).expect("formatting should succeed").text;

        assert_eq!(perturbed_formatted, formatted);
        assert_eq!(second_pass, formatted);
        assert!(!profile_source(&formatted).has_parse_errors);
    }

    #[test]
    fn formats_multiline_multi_target_animate_targets_compactly() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component MainWindow inherits Rectangle {\n    animate x , y {\n        duration: 170ms;\n        easing: cubic-bezier(0.17,0.76,0.4,1.75);\n    }\n}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "export component MainWindow inherits Rectangle {\n    animate x, y {\n        duration: 170ms;\n        easing: cubic-bezier(0.17, 0.76, 0.4, 1.75);\n    }\n}\n"
        );
        assert!(!profile_source(&result.text).has_parse_errors);
    }

    #[test]
    fn strict_formatting_accepts_multi_target_animate_blocks() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component TestCase { animate background, border-color { duration: 120ms; } }";
        let strict =
            formatter.format_with_operation(input, default_operation()).expect("strict output");

        assert_eq!(
            strict,
            "component TestCase { animate background, border-color { duration: 120ms; } }\n"
        );
        assert!(!profile_source(&strict).has_parse_errors);
    }

    #[test]
    fn strict_formatting_accepts_wildcard_animate_blocks() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component TestCase { animate * { duration: 120ms; easing: ease-out; } }";
        let strict =
            formatter.format_with_operation(input, default_operation()).expect("strict output");

        assert!(strict.contains("animate * {"));
        assert!(!profile_source(&strict).has_parse_errors);
    }

    #[test]
    fn strict_formatting_compacts_multiline_multi_target_animate_blocks() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component MainWindow inherits Rectangle {\n    animate x , y {\n        duration: 170ms;\n        easing: cubic-bezier(0.17,0.76,0.4,1.75);\n    }\n}";
        let strict =
            formatter.format_with_operation(input, default_operation()).expect("strict output");

        assert_eq!(
            strict,
            "export component MainWindow inherits Rectangle {\n    animate x, y {\n        duration: 170ms;\n        easing: cubic-bezier(0.17, 0.76, 0.4, 1.75);\n    }\n}\n"
        );
        assert!(!profile_source(&strict).has_parse_errors);
    }

    #[test]
    fn formats_ternary_expressions_with_spaces() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component TestCase inherits Rectangle { background: condition?#373737:#ffffff; }";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "export component TestCase inherits Rectangle { background: condition ? #373737 : #ffffff; }\n"
        );
        assert!(!profile_source(&result.text).has_parse_errors);
    }

    #[test]
    fn strict_formatting_accepts_states_without_when_clauses() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component LspCrashMvp { states [ active: { } inactive: { } ] }";
        let strict =
            formatter.format_with_operation(input, default_operation()).expect("strict output");

        assert!(strict.contains("states ["));
        assert!(!profile_source(&strict).has_parse_errors);
    }
}

#[test]
fn handles_empty_element_block() {
    let formatter = Formatter::new().expect("formatter should initialize");
    let input = "export component EmptyBlock { Rectangle {} }";
    let result = formatter.format_str(input).expect("formatting should succeed");

    // Empty blocks should remain inline
    assert_eq!(result.text, "export component EmptyBlock { Rectangle {} }\n");
}

#[test]
fn handles_empty_callback_block() {
    let formatter = Formatter::new().expect("formatter should initialize");
    let input = "export component TestCallback { callback foo; foo => {} }";
    let result = formatter.format_str(input).expect("formatting should succeed");

    // Empty callback blocks should remain inline
    assert!(result.text.contains("foo => {}"));
}
