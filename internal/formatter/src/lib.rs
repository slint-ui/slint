// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod input_diagnostics;
pub mod profiling;
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
    InvalidInput { diagnostics: String, source: topiary_core::FormatterError },
    Formatter(topiary_core::FormatterError),
    UnsupportedPath(std::path::PathBuf),
}

impl Formatter {
    pub fn new() -> Result<Self, FormatError> {
        Ok(Self { language: slint_language()? })
    }

    pub fn format_str(&self, source: &str) -> Result<FormatResult, FormatError> {
        let text = self.format_source(source, None)?;
        Ok(FormatResult { changed: text != source, text })
    }

    pub fn format_path(&self, path: impl AsRef<Path>) -> Result<FormatResult, FormatError> {
        let path = path.as_ref();
        if path.extension().and_then(|extension| extension.to_str()) != Some("slint") {
            return Err(FormatError::UnsupportedPath(path.to_owned()));
        }

        let source = fs::read_to_string(path)?;
        let text = self.format_source(&source, Some(path))?;
        Ok(FormatResult { changed: text != source, text })
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

impl Formatter {
    fn format_source(&self, source: &str, path: Option<&Path>) -> Result<String, FormatError> {
        match self.format_with_operation(source, default_operation()) {
            Ok(text) => Ok(text),
            Err(error) => {
                if let Some(diagnostics) =
                    input_diagnostics::compiler_diagnostics_for_broken_input(source, path)
                {
                    Err(FormatError::InvalidInput { diagnostics, source: error })
                } else {
                    Err(error.into())
                }
            }
        }
    }

    fn format_with_operation(
        &self,
        source: &str,
        operation: Operation,
    ) -> Result<String, topiary_core::FormatterError> {
        let mut output = Vec::new();
        topiary_core::formatter_str(source, &mut output, &self.language, operation)?;
        Ok(String::from_utf8(output).expect("topiary should emit valid UTF-8"))
    }
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => err.fmt(f),
            Self::InvalidInput { diagnostics, .. } => diagnostics.fmt(f),
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
            Self::InvalidInput { source, .. } => Some(source),
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
    use super::{FormatError, Formatter, default_operation, profile_source};

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
    fn keeps_semicolons_at_line_end_in_multiline_callback_blocks() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component Test {\n    callback test();\n    test => {\n        self.x = 1;\n        self.y = 2;\n        self.z = 3;\n    }\n}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "export component Test {\n    callback test();\n    test => {\n        self.x = 1;\n        self.y = 2;\n        self.z = 3;\n    }\n}\n"
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
    fn keeps_top_level_element_definitions_flush_left_after_blank_lines() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "First := Rectangle {}\n\nSecond := Rectangle {\n    x: 42px;\n}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "First := Rectangle {}\n\nSecond := Rectangle {\n    x: 42px;\n}\n"
        );
    }

    #[test]
    fn preserves_blank_lines_around_top_level_section_comments() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component Helper inherits Rectangle {}\n\n//------ Widgets ------\n\nexport component TestCase inherits Rectangle {}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "component Helper inherits Rectangle {}\n\n//------ Widgets ------\n\nexport component TestCase inherits Rectangle {}\n"
        );
    }

    #[test]
    fn keeps_top_level_imports_on_their_own_line_after_definitions() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component Helper inherits Rectangle {}\n\nimport { Button } from \"std-widgets.slint\";";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "component Helper inherits Rectangle {}\n\nimport { Button } from \"std-widgets.slint\";\n"
        );
    }

    #[test]
    fn formats_export_statements_before_definitions() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export {Foo as Bar} from \"./x.slint\";\ncomponent Demo{}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "export { Foo as Bar } from \"./x.slint\";\n\ncomponent Demo {}\n"
        );
    }

    #[test]
    fn preserves_rust_attr_blocks_while_spacing_top_level_sections() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "@rust-attr(derive(Debug))\nexport struct Foo {}\ncomponent Demo{}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "@rust-attr(derive(Debug))\nexport struct Foo {}\n\ncomponent Demo {}\n"
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
    fn preserves_blank_lines_before_multiline_block_closing_braces() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "component TestCase {\n    Rectangle {\n        x: 42px;\n    }\n\n}";
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "component TestCase {\n    Rectangle {\n        x: 42px;\n    }\n\n}\n"
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
    fn strict_formatting_preserves_spaces_before_member_access_on_int_literals() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component TestCase inherits Window { out property <bool> test: 100 .sqrt() == 10 && 4 .log(2) == 2 && 0 .abs() == 0; }";
        let strict =
            formatter.format_with_operation(input, default_operation()).expect("strict output");

        assert!(strict.contains("100 .sqrt()"));
        assert!(strict.contains("4 .log(2)"));
        assert!(strict.contains("0 .abs()"));
        assert!(!profile_source(&strict).has_parse_errors);
    }

    #[test]
    fn strict_formatting_compacts_member_access_for_non_int_expression_bases() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = r#"export component TestCase inherits Window {
    out property <string> keys_member: @keys(Control + A) .to-string();
    out property <string> tr_member: @tr("hello") .to-uppercase();
    out property <bool> markdown_member: @markdown("hello") .baz;
    out property <brush> gradient_member: @linear-gradient(45deg, red, blue) .darker(10%);
    out property <bool> image_member: @image-url("cat.jpg") .baz;
    out property <int> index_member: [1, 2][0] .abs();
    out property <int> list_member: [1, 2] .length;
    out property <bool> struct_member: { foo: true } .foo;
}"#;
        let strict =
            formatter.format_with_operation(input, default_operation()).expect("strict output");

        let member_line = |name: &str| {
            strict
                .lines()
                .find(|line| line.contains(&format!("{name}:")))
                .unwrap_or_else(|| panic!("missing formatted line for {name}:\n{strict}"))
        };

        assert!(member_line("keys_member").contains(").to-string();"));
        assert!(!member_line("keys_member").contains(") .to-string();"));
        assert!(member_line("tr_member").contains(").to-uppercase();"));
        assert!(!member_line("tr_member").contains(") .to-uppercase();"));
        assert!(member_line("markdown_member").contains(").baz;"));
        assert!(!member_line("markdown_member").contains(") .baz;"));
        assert!(member_line("gradient_member").contains(").darker(10%);"));
        assert!(!member_line("gradient_member").contains(") .darker(10%);"));
        assert!(member_line("image_member").contains(").baz;"));
        assert!(!member_line("image_member").contains(") .baz;"));
        assert!(member_line("index_member").contains("].abs();"));
        assert!(!member_line("index_member").contains("] .abs();"));
        assert!(member_line("list_member").contains("].length;"));
        assert!(!member_line("list_member").contains("] .length;"));
        assert!(member_line("struct_member").contains("}.foo;"));
        assert!(!member_line("struct_member").contains("} .foo;"));
        let profile = profile_source(&strict);
        assert!(!profile.has_parse_errors, "{:?}", profile.diagnostics);
    }

    #[test]
    fn formats_changed_callbacks_events_and_member_access_without_parse_errors() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = r#"component Base { callback changed(int); }
component Demo {
    base:=Base{}
    callback changed<=> base . changed;
    in-out property<int> value;
    changed(delta)=>{ root.changed(+1); base.changed(delta); }
    changed value=>{ root.changed(value);}
}"#;
        let result = formatter.format_str(input).expect("formatting should succeed");

        assert_eq!(
            result.text,
            "component Base { callback changed(int); }\n\ncomponent Demo {\n    base := Base {}\n    callback changed <=> base.changed;\n    in-out property <int> value;\n    changed(delta) => { root.changed(+ 1); base.changed(delta); }\n    changed value => { root.changed(value); }\n}\n"
        );

        let profile = profile_source(&result.text);
        assert!(!profile.has_parse_errors, "{:?}", profile.diagnostics);
    }

    #[test]
    fn reports_compiler_diagnostics_for_invalid_input() {
        let formatter = Formatter::new().expect("formatter should initialize");
        let input = "export component Broken inherits Rectangle { @@@ }";
        let error = formatter.format_str(input).expect_err("invalid input should fail");

        match error {
            FormatError::InvalidInput { diagnostics, .. } => {
                assert!(diagnostics.contains("error:"));
                assert!(diagnostics.contains("<input>.slint"));
            }
            other => panic!("expected invalid input diagnostics, got {other:?}"),
        }
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
