// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `layout-order` is stable API while `cross-axis-self-alignment` is experimental.
//! The syntax tests can't verify that because they always enable the experimental
//! features, so these tests compile with them disabled.

use i_slint_compiler::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

const FLEX_ITEM_PROPERTIES: &[&str] = &["layout-order: 3"];

/// Compile `source` without experimental features and return its errors
/// (warnings are not of interest here).
fn errors(source: String) -> Vec<String> {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source, None, &mut diagnostics);
    let mut config = CompilerConfiguration::new(OutputFormat::Interpreter);
    // The default follows SLINT_ENABLE_EXPERIMENTAL_FEATURES; pin it off.
    config.enable_experimental = false;
    let (_, diagnostics, _) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));
    diagnostics
        .iter()
        .filter(|d| d.level() == DiagnosticLevel::Error)
        .map(|d| d.message().to_owned())
        .collect()
}

fn in_flexbox(binding: &str) -> Vec<String> {
    errors(format!(
        r#"
export component TestCase inherits Window {{
    FlexboxLayout {{
        Rectangle {{ {binding}; }}
    }}
}}
"#
    ))
}

fn name_of(binding: &str) -> &str {
    binding.split(':').next().unwrap()
}

#[test]
fn flex_item_properties_are_stable() {
    for binding in FLEX_ITEM_PROPERTIES {
        assert_eq!(in_flexbox(binding), Vec::<String>::new());
    }
}

/// `cross-axis-self-alignment` is experimental in every layout, pending optional types.
#[test]
fn cross_axis_self_alignment_is_experimental() {
    for layout in ["FlexboxLayout", "HorizontalLayout", "VerticalLayout"] {
        let source = format!(
            r#"
export component TestCase inherits Window {{
    {layout} {{
        Rectangle {{ cross-axis-self-alignment: center; }}
    }}
}}
"#
        );
        assert_eq!(errors(source), ["'cross-axis-self-alignment' is an experimental feature"]);
    }
}

/// In a GridLayout (or outside of any layout) the property is rejected.
#[test]
fn cross_axis_self_alignment_rejected_in_grid() {
    let source = r#"
export component TestCase inherits Window {
    GridLayout {
        Rectangle { cross-axis-self-alignment: center; }
    }
}
"#;
    assert_eq!(
        errors(source.into()),
        [
            "cross-axis-self-alignment used outside of a FlexboxLayout, HorizontalLayout, or VerticalLayout"
        ]
    );
}

#[test]
fn used_outside_of_a_flexbox_is_rejected() {
    for binding in FLEX_ITEM_PROPERTIES {
        let source = format!(
            r#"
export component TestCase inherits Window {{
    VerticalLayout {{
        Rectangle {{ {binding}; }}
    }}
}}
"#
        );
        assert_eq!(
            errors(source),
            [format!("{} used outside of a FlexboxLayout", name_of(binding))]
        );
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
    VerticalLayout {{
        {cell} Rectangle {{ layout-order: 1; }}
    }}
}}
"#
        );
        assert_eq!(errors(source), ["layout-order used outside of a FlexboxLayout"]);
    }
}

/// The `CrossAxisSelfAlignment` enum is not in the stable type register.
#[test]
fn cross_axis_self_alignment_enum_is_experimental() {
    let source = r#"
export component TestCase inherits Window {
    in property <CrossAxisSelfAlignment> a;
}
"#;
    assert_eq!(errors(source.into()), ["Unknown type 'CrossAxisSelfAlignment'"]);
}
