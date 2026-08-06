// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The per-item `flex-*` properties of a `FlexboxLayout` are not stable API yet, so they are only
//! accepted with experimental features enabled. The syntax tests can't cover this because they
//! always enable them. This also hosts the stable-API tests for `cross-axis-self-alignment`,
//! which needs the same experimental-features control.

use i_slint_compiler::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

const FLEX_ITEM_PROPERTIES: &[&str] =
    &["flex-grow: 1", "flex-shrink: 2", "flex-basis: 100px", "flex-order: 3"];

/// Compile `source` and return its errors (warnings are not of interest here).
fn errors(source: String, enable_experimental: bool) -> Vec<String> {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source, None, &mut diagnostics);
    let mut config = CompilerConfiguration::new(OutputFormat::Interpreter);
    config.enable_experimental = enable_experimental;
    let (_, diagnostics, _) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));
    diagnostics
        .iter()
        .filter(|d| d.level() == DiagnosticLevel::Error)
        .map(|d| d.message().to_owned())
        .collect()
}

fn in_flexbox(binding: &str, enable_experimental: bool) -> Vec<String> {
    errors(
        format!(
            r#"
export component TestCase inherits Window {{
    FlexboxLayout {{
        Rectangle {{ {binding}; }}
    }}
}}
"#
        ),
        enable_experimental,
    )
}

fn name_of(binding: &str) -> &str {
    binding.split(':').next().unwrap()
}

#[test]
fn flex_item_properties_are_experimental() {
    for binding in FLEX_ITEM_PROPERTIES {
        assert_eq!(
            in_flexbox(binding, false),
            [format!("'{}' is an experimental feature", name_of(binding))]
        );
    }
}

#[test]
fn flex_item_properties_accepted_with_experimental() {
    for binding in FLEX_ITEM_PROPERTIES {
        assert_eq!(in_flexbox(binding, true), Vec::<String>::new());
    }
}

/// `cross-axis-self-alignment` is stable API, usable without experimental features.
#[test]
fn cross_axis_self_alignment_is_stable() {
    assert_eq!(in_flexbox("cross-axis-self-alignment: center", false), Vec::<String>::new());
}

/// Outside of a `FlexboxLayout` the property is wrong regardless of the experimental features, so
/// only that error is reported: being sent to a construct that would then reject them for another
/// reason would be confusing.
#[test]
fn used_outside_of_a_flexbox_reports_only_that() {
    for binding in
        FLEX_ITEM_PROPERTIES.iter().chain(std::iter::once(&"cross-axis-self-alignment: center"))
    {
        let source = format!(
            r#"
export component TestCase inherits Window {{
    VerticalLayout {{
        Rectangle {{ {binding}; }}
    }}
}}
"#
        );
        let expected = [format!("{} used outside of a FlexboxLayout", name_of(binding))];
        assert_eq!(errors(source.clone(), false), expected);
        assert_eq!(errors(source, true), expected);
    }
}

/// A repeated or conditional cell gets an element injected around it, onto which the `flex-*`
/// bindings are linked. Check that this doesn't report the error a second time.
#[test]
fn repeated_and_conditional_cells_report_once() {
    for cell in ["for i in 3:", "if true:"] {
        let source = format!(
            r#"
export component TestCase inherits Window {{
    FlexboxLayout {{
        {cell} Rectangle {{ flex-grow: 1; }}
    }}
}}
"#
        );
        assert_eq!(errors(source, false), ["'flex-grow' is an experimental feature"]);
    }
}

/// The `CrossAxisSelfAlignment` enum is in the stable type register:
/// nameable as a type and as a qualified value without experimental features.
#[test]
fn cross_axis_self_alignment_enum_is_stable() {
    let source = r#"
export component TestCase inherits Window {
    in property <CrossAxisSelfAlignment> a: CrossAxisSelfAlignment.center;
    FlexboxLayout {
        Rectangle { cross-axis-self-alignment: root.a; }
    }
}
"#;
    assert_eq!(errors(source.into(), false), Vec::<String>::new());
}
